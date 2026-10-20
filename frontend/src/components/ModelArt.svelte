<script lang="ts">
  let { color = '#198754' }: { color?: string } = $props();
  const id = $props.id();
</script>

<svg
  class="model-art"
  viewBox="0 0 540 420"
  role="img"
  aria-label="Ilustracja przykładowej części drukowanej w 3D"
>
  <defs>
    <linearGradient id={`${id}-top`} x1="0" y1="0" x2="1" y2="1"
      ><stop stop-color="#c7ffda" /><stop offset=".25" stop-color={color} /><stop
        offset="1"
        stop-color={color}
      /></linearGradient
    >
    <linearGradient id={`${id}-front`} x2="0" y2="1"
      ><stop stop-color={color} /><stop offset="1" stop-color="#092c20" /></linearGradient
    >
    <pattern id={`${id}-lines`} width="5" height="5" patternUnits="userSpaceOnUse"
      ><path d="M0 0h5" stroke="#ffffff" stroke-opacity=".12" stroke-width="1" /></pattern
    >
    <filter id={`${id}-shadow`}><feGaussianBlur stdDeviation="15" /></filter>
  </defs>
  <ellipse
    cx="275"
    cy="335"
    rx="140"
    ry="28"
    fill="#000"
    opacity=".5"
    filter={`url(#${id}-shadow)`}
  />
  <g transform="translate(0 -7)">
    <path d="m136 254 153 80 123-72v-50l-153 75-123-65z" fill={`url(#${id}-front)`} />
    <path d="m136 222 123-72 153 80-123 72z" fill={`url(#${id}-top)`} />
    <path d="m136 254 153 80 123-72v-32l-123 72-153-80z" fill={`url(#${id}-lines)`} />
    <path d="m136 222 153 80v32l-153-80z" fill="#0e4d34" opacity=".5" />
    <path d="m244 170 44 24 62-36V98l-44-24-62 36z" fill={`url(#${id}-front)`} />
    <path d="m244 110 44 24 62-36-44-24z" fill={`url(#${id}-top)`} />
    <path d="m288 134 62-36v60l-62 36z" fill="#0a3f2b" />
    <path d="m244 170 44 24 62-36V98l-62 36-44-24z" fill={`url(#${id}-lines)`} />
    <path d="m244 170 44 24v84l-44-23z" fill={`url(#${id}-front)`} />
    <path d="m288 194 62-36v84l-62 36z" fill="#11653e" />
    <path d="m288 194 62-36v84l-62 36z" fill={`url(#${id}-lines)`} />
    <ellipse cx="207" cy="228" rx="17" ry="9" transform="rotate(27 207 228)" fill="#0b3c28" />
    <ellipse cx="333" cy="254" rx="17" ry="9" transform="rotate(-30 333 254)" fill="#0b3c28" />
    <path
      d="m140 221 149 78 119-69M246 109l42 23 59-34"
      fill="none"
      stroke="#d0ffde"
      stroke-opacity=".45"
    />
  </g>
  <g stroke="#718d7b" stroke-width="1" fill="none" opacity=".6"
    ><path d="m115 273 153 80m-155-86-6 12m155 68-6 12M370 94h40m-25 0v160m-7 0h14" /></g
  >
  <g fill="#a2afa6" font-size="10" font-family="monospace"
    ><text x="155" y="338" transform="rotate(27 155 338)">86.00 mm</text><text
      x="402"
      y="189"
      transform="rotate(-90 402 189)">42.00 mm</text
    ></g
  >
</svg>
