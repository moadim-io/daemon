---
"moadim": patch
---

### Changed

- **Hardened the dashboard's Content-Security-Policy.** Every response's CSP
  previously carried only `frame-ancestors 'none'` (#406's anti-clickjacking
  fix), leaving `script-src`/`style-src`/`default-src` unset and an injected
  inline `<script>` or `<base>` tag entirely unblocked — a real gap given the
  dashboard drives an unauthenticated loopback API with destructive controls
  (create/trigger/delete routines, `POST /shutdown`). The CSP now sets
  `default-src 'self'` and explicit `script-src`, `style-src`, `font-src`,
  `img-src`, `connect-src`, `base-uri 'none'`, `form-action 'none'`, and
  `object-src 'none'` directives verified against the bundled Yew/WASM SPA and
  Swagger UI, while keeping `frame-ancestors 'none'`. (#551)
