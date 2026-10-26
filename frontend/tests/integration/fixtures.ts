import { test as base, expect } from '@playwright/test';

// Fail on uncaught browser errors; report only names, never values from auth/payment forms.
export const test = base.extend<{ browserErrors: void }>({
  browserErrors: [
    async ({ page, context }, use) => {
      const errors: string[] = [];
      const observe = (target: typeof page) =>
        target.on('pageerror', (error) => errors.push(error.name));
      observe(page);
      context.on('page', observe);
      await use();
      expect(errors, 'No uncaught JavaScript errors in the browser').toEqual([]);
    },
    { auto: true },
  ],
});
export { expect };

export type { Page, Frame, APIRequestContext } from '@playwright/test';
