import { randomUUID } from 'node:crypto';
import { expect, type Page } from './fixtures';

// Bound the work explicitly: growing beyond these limits must be reviewed, not skipped.
const maxSvgRoots = 160;
const maxShapesPerSvg = 2_000;
const pixelProofSize = 600;

/** Large page backgrounds receive geometry checks only; bounded SVGs also need paint proof. */
export async function checkSvgGraphics(page: Page, label: string, minimumPainted = 1) {
  const svgs = page.locator('svg');
  const total = await svgs.count();
  expect(total, `${label}: SVG count exceeds the rendering-check budget`).toBeLessThanOrEqual(
    maxSvgRoots,
  );
  const counts = { total, hidden: 0, geometry: 0, painted: 0, largeGeometryOnly: 0 };
  for (let index = 0; index < total; index++) {
    const svg = svgs.nth(index);
    const description = `${label}: SVG ${index}`;
    const state = await svg.evaluate((node, shapeLimit) => {
      function hidden(element: Element) {
        for (let current: Element | null = element; current; current = current.parentElement) {
          const style = getComputedStyle(current);
          if (
            style.display === 'none' ||
            style.visibility === 'hidden' ||
            style.visibility === 'collapse' ||
            Number(style.opacity) === 0
          )
            return true;
        }
        return false;
      }
      function finiteBox(box: DOMRect | SVGRect, viewport = false) {
        return (
          [box.x, box.y, box.width, box.height].every(Number.isFinite) &&
          box.width >= 0 &&
          box.height >= 0 &&
          (viewport ? box.width > 0 && box.height > 0 : box.width > 0 || box.height > 0)
        );
      }
      if (hidden(node)) return { hidden: true };
      const viewport = node.getBoundingClientRect();
      const shapes = [
        ...node.querySelectorAll(
          'path,rect,circle,ellipse,line,polyline,polygon,text,use,image,foreignObject',
        ),
      ].filter((shape) => !shape.closest('defs,clipPath,mask,pattern,marker,symbol'));
      if (shapes.length > shapeLimit) return { hidden: false, overBudget: shapes.length };
      const geometry =
        node instanceof SVGSVGElement &&
        finiteBox(node.getBBox()) &&
        shapes.some(
          (shape) =>
            shape instanceof SVGGraphicsElement && !hidden(shape) && finiteBox(shape.getBBox()),
        );
      return {
        hidden: false,
        viewport: finiteBox(viewport, true),
        geometry,
        width: viewport.width,
        height: viewport.height,
      };
    }, maxShapesPerSvg);
    if (state.hidden) {
      counts.hidden++;
      continue;
    }
    expect(
      state.overBudget,
      `${description}: shape count exceeds the rendering-check budget`,
    ).toBeUndefined();
    expect(state.viewport, `${description}: finite positive SVG viewport`).toBe(true);
    expect(state.geometry, `${description}: nonempty finite SVG geometry outside definitions`).toBe(
      true,
    );
    counts.geometry++;
    if (state.width! > pixelProofSize || state.height! > pixelProofSize) {
      counts.largeGeometryOnly++;
      continue;
    }

    // Hide only this SVG's descendants: another SVG/background cannot supply its proof.
    const marker = `svg-paint-${randomUUID()}`;
    const previous = await svg.getAttribute('data-e2e-svg');
    await svg.evaluate((node, value) => node.setAttribute('data-e2e-svg', value), marker);
    try {
      // Opaque diagnostic background prevents motion behind a transparent SVG from faking paint.
      const background = `[data-e2e-svg="${marker}"] { background: #ff00ff !important; }`;
      const painted = await svg.screenshot({ animations: 'disabled', style: background });
      const blank = await svg.screenshot({
        animations: 'disabled',
        style: `${background} [data-e2e-svg="${marker}"] * { visibility: hidden !important; }`,
      });
      expect(
        painted.equals(blank),
        `${description}: SVG descendants must paint visible pixels`,
      ).toBe(false);
      counts.painted++;
    } finally {
      await svg.evaluate((node, value) => {
        if (value === null) node.removeAttribute('data-e2e-svg');
        else node.setAttribute('data-e2e-svg', value);
      }, previous);
    }
  }
  expect(counts.painted, `${label}: expected visible painted SVGs`).toBeGreaterThanOrEqual(
    minimumPainted,
  );
  return counts;
}
