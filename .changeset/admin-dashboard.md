---
canvist_cloud: feat
website: feat
---

# Add the admin dashboard, Stripe coupons, and first-party analytics

`canvist.dev/admin` is the admin dashboard. It has analytics, Stripe coupons, and projects.

- **Sign-in by email:** only addresses in `CANVIST_ADMIN_EMAILS` are sent a 15-minute link, through Resend (or the server log on localhost). The link is traded for a week-long `HttpOnly`, `SameSite=Strict` session cookie, signed with a key derived from `CANVIST_TOKEN_SECRET`. No session is stored, and removing an address locks it out. `CANVIST_ADMIN_TOKEN` still works for scripts.
- **Coupons:** `POST /v1/admin/coupons` creates a Stripe coupon and its promotion code.
  - A public code works for anyone at Checkout.
  - A private code is bound to one project's Stripe customer, and is applied to the project's subscription if it already pays.
  - The dashboard has presets for free forever, open source, free trials, small teams, and launch codes.
  - Monthly Checkout now accepts promotion codes.
- **Analytics:** the website sends page views and clicks on `data-capture` elements to `POST /v1/events`. It sends no query strings and stores no addresses; one address is read, in memory, only to rate-limit a client's batches. Bots are dropped, and browsers that ask not to be tracked send nothing. `GET /v1/admin/analytics` reports visitors, sessions, page views, bounce rate, time per session, daily visitors and top lists against the period before. The server records `project created` and `checkout started` itself.
- **Projects:** `GET /v1/admin/projects` lists projects with their plan and this month's usage. The existing admin routes moved into the admin module unchanged.
- **Proxy:** Caddy proxies `/v1/admin` and `/v1/events` to the cloud service at `CANVIST_CLOUD_URL`, so the dashboard's cookie stays first-party. `astro dev` does the same.
