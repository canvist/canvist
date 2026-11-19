# Self-hosting

In this guide you run canvist Cloud's server yourself: the same binary canvist Cloud runs, free, with no caps. You'll start it, create a project, configure it, and learn how it stores documents and scales.

## 1. Start the server

The server is one binary, `canvist-cloud`, in `services/cloud`. Build its image from the repository root, then run it with a token secret, an admin token, and a Postgres database:

```sh
docker build -f services/cloud/Dockerfile -t canvist-cloud .
docker run -p 8080:8080 \
  -e CANVIST_TOKEN_SECRET="$(openssl rand -hex 32)" \
  -e CANVIST_ADMIN_TOKEN="$(openssl rand -hex 32)" \
  -e DATABASE_URL=postgres://user:password@db.internal:5432/canvist \
  canvist-cloud
```

Or run it from source:

```sh
CANVIST_TOKEN_SECRET=$(openssl rand -hex 32) \
CANVIST_ADMIN_TOKEN=$(openssl rand -hex 32) \
DATABASE_URL=postgres://user:password@localhost:5432/canvist \
cargo run --release -p canvist_cloud
```

Without `DATABASE_URL`, documents live in memory and vanish on restart, which is only useful for trying it out. Database migrations run on startup.

## 2. Create a project

Create a project with the admin token. It returns one API key per environment, shown once:

```sh
curl -X POST http://localhost:8080/v1/admin/projects \
  -H "Authorization: Bearer $CANVIST_ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{ "name": "my-app", "environments": ["production", "staging"] }'
```

New projects start on the free plan's 1,000-document cap. Remove the cap:

```sh
curl -X PUT http://localhost:8080/v1/admin/projects/$PROJECT/terms \
  -H "Authorization: Bearer $CANVIST_ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{ "plan": "enterprise" }'
```

Other admin routes: `POST /v1/admin/projects/{project}/keys` with `{ "environment": "staging" }` adds a key, `GET /v1/admin/projects` lists projects with this month's usage, and `GET /v1/admin/stats` shows loaded documents and connected clients.

## The admin dashboard

canvist.dev serves an admin dashboard at `/admin`: analytics, Stripe coupons, and projects. It signs people in by email rather than with the admin token:

1. Set `CANVIST_ADMIN_EMAILS` to the addresses that may sign in, and `CANVIST_ADMIN_URL` to the dashboard's address.
2. Someone enters their address at `/admin`. Only a listed address is sent a link, but the page answers the same way for any address. The link works for 15 minutes.
3. The link's token sits in the URL fragment, so it never reaches a server log. The page trades it for a session cookie that lasts a week (`HttpOnly`, `SameSite=Strict`, scoped to `/v1/admin`).

Nothing about sessions is stored. Removing an address from `CANVIST_ADMIN_EMAILS` locks it out at its next request, and rotating `CANVIST_TOKEN_SECRET` signs everyone out. A cookie-authenticated request from another origin is refused.

Links go out through [Resend](https://resend.com) (`RESEND_API_KEY`, `CANVIST_MAIL_FROM`). When `CANVIST_ADMIN_URL` is on `localhost`, the server writes them to its log instead. Anywhere else it refuses to start without a mailer.

The website's Caddy server proxies `/v1/admin` and `/v1/events` to the cloud service at `CANVIST_CLOUD_URL`, so the dashboard and its cookie stay on one origin. `deno task dev` proxies to `http://localhost:8080`.

### Coupons

`POST /v1/admin/coupons` creates a Stripe coupon and one promotion code for it:

```json
{
	"name": "Free forever",
	"percent_off": 100,
	"duration": "forever",
	"visibility": "private",
	"project": "prj_k3f9a2m1q8x7"
}
```

- **The discount** is either `percent_off` (above 0, at most 100), or `amount_off` in cents together with a `currency`.
- **The duration** is `once`, `forever`, or `repeating` with `duration_in_months` (1 to 36).
- **A public coupon** gets a code anyone can enter at Checkout. Pass `code`, or Stripe makes one up.
- **A private coupon** gets a code bound to the project's Stripe customer; a project without a customer gets one. If the project already pays, the code is applied to its subscription straight away.
- **Limits:** `max_redemptions` and `expires_at` (Unix seconds) limit the code.

A free trial that still collects a card is 100% off for a few months.

Monthly Checkout accepts promotion codes. Annual Checkout saves a card and charges nothing, so it takes no code; apply a private coupon to the subscription once it exists.

`GET /v1/admin/coupons` lists the newest 100 coupons with their codes. `DELETE /v1/admin/coupons/{id}` switches off a coupon's codes and deletes it; subscriptions that already use it keep the discount until it ends.

### Analytics

The website sends a page view per page, and a click on any element marked `data-capture="name"`, to `POST /v1/events`. Events land in the `analytics_events` table:

- No IP addresses are read and query strings are never sent.
- A visitor is a random id kept in the browser's storage.
- Bots are dropped by user agent.
- Browsers that send Global Privacy Control or Do Not Track send nothing.

The server also records `project created` and `checkout started`.

`GET /v1/admin/analytics?days=30` reports the last 1 to 90 days against the period before:

- visitors, sessions, page views, bounce rate, and time per session
- visitors per day
- the top pages, sources, devices, and browsers
- the site's clicks, and the server's events

It reads at most 200,000 events and says `truncated` when it stops there.

Then follow [Sync](./sync.md) and [Convert](./convert.md) with `localhost:8080` as the host.

## Configuration

| Variable                        | Default                   | Meaning                                                                                           |
| ------------------------------- | ------------------------- | ------------------------------------------------------------------------------------------------- |
| `CANVIST_TOKEN_SECRET`          | required                  | HMAC key for client tokens, at least 32 bytes. Rotating it signs everyone out.                    |
| `DATABASE_URL`                  | memory                    | Postgres connection string. Connections are unencrypted: use a private network or a TLS proxy.    |
| `CANVIST_ADMIN_TOKEN`           | off                       | Bearer token for `/v1/admin`, for scripts.                                                        |
| `CANVIST_ADMIN_EMAILS`          | off                       | Comma-separated addresses that may sign in to the admin dashboard.                                |
| `CANVIST_ADMIN_URL`             | required with emails      | The dashboard's address, such as `https://canvist.dev/admin`. Sign-in links point at it.          |
| `RESEND_API_KEY`                | off                       | Sends sign-in links through Resend. Required with emails unless the dashboard is on `localhost`.  |
| `CANVIST_MAIL_FROM`             | required with Resend      | The sender, such as `canvist <admin@canvist.dev>`, on a domain verified with Resend.              |
| `HOST`, `PORT`                  | `::`, `8080`              | Where to listen. `::` accepts IPv4 and IPv6.                                                      |
| `CANVIST_ROLE`                  | `all`                     | `all`, `sync`, or `convert`: which route groups this instance serves.                             |
| `CANVIST_IDLE_EVICTION_SECONDS` | `60`                      | How long a document with no connections stays in memory.                                          |
| `CANVIST_FLUSH_INTERVAL_MS`     | `250`                     | How often changes are written to Postgres.                                                        |
| `CANVIST_COMPACT_AFTER_UPDATES` | `200`                     | Update rows after which a document is folded into a snapshot.                                     |
| `CANVIST_MAX_MESSAGE_BYTES`     | `1048576`                 | The largest WebSocket message and REST body.                                                      |
| `CANVIST_MAX_DOCUMENT_BYTES`    | `16777216`                | The largest document, as encoded Yjs state.                                                       |
| `CANVIST_WEBHOOK_DEBOUNCE_MS`   | `2000`                    | Quiet time before a `document.updated` webhook.                                                   |
| `CANVIST_WEBHOOK_MAX_WAIT_MS`   | `30000`                   | The longest a webhook waits while changes keep coming.                                            |
| `CANVIST_WEBHOOK_ALLOW_PRIVATE` | `false`                   | Allow HTTP and private hosts as webhook targets, inside your network.                             |
| `CANVIST_CONVERT_MAX_BYTES`     | `8388608`                 | The largest conversion request.                                                                   |
| `CANVIST_CONVERT_TIMEOUT_MS`    | `10000`                   | The longest a conversion may run before the request gives up.                                     |
| `CANVIST_CONVERT_CONCURRENCY`   | twice the CPUs            | Conversions running at once.                                                                      |
| `CANVIST_PAUSE_RECHECK_SECONDS` | `30`                      | How often a session paused by a full free plan checks again.                                      |
| `STRIPE_SECRET_KEY`             | off                       | A Stripe **test-mode** key (`sk_test_` or `rk_test_`); live keys are refused.                     |
| `STRIPE_WEBHOOK_SECRET`         | off                       | The Stripe webhook endpoint's signing secret; with the key, turns on `POST /v1/stripe/webhook`.   |
| `STRIPE_METER_EVENT`            | `canvist_active_document` | The monthly active document meter's event name.                                                   |
| `STRIPE_CONVERSION_METER_EVENT` | `canvist_conversion`      | The conversion meter's event name.                                                                |
| `STRIPE_REPORT_INTERVAL_MS`     | `10000`                   | How often unreported usage is sent to Stripe.                                                     |
| `STRIPE_AUTOMATIC_TAX`          | `false`                   | Calculate tax in Checkout. Only with a head office address and an active Stripe Tax registration. |
| `CANVIST_LOG_FORMAT`            | text                      | `json` for structured logs.                                                                       |
| `RUST_LOG`                      | `info`                    | Log filter, such as `canvist_cloud=debug`.                                                        |

To bill your own customers through Stripe, set the `STRIPE_*` variables on one service per database. Test-mode keys only: the server refuses live keys.

## How documents are stored

Changes are written behind: every flush interval, all edited documents are saved in one transaction, and folded into a snapshot once enough build up, when a document goes idle, and at shutdown. If the process dies between flushes, clients send the missing changes again when they reconnect, because every sync session starts by exchanging what each side lacks.

On `SIGTERM`, the server stops reporting ready, asks every client to reconnect (close code `1012`), and saves every loaded document. Give it at least 15 seconds before a hard kill.

## Scaling

Each instance holds the documents it serves in memory, so every client of one document must reach the same instance. Run one sync instance, or route by document name. Conversions are stateless and scale out freely.

What one instance handles, measured on Apple silicon:

| Measure                                   | Measured |
| ----------------------------------------- | -------- |
| One keystroke to the last of 100 peers    | 0.6 ms   |
| Applying one keystroke                    | 1.5 µs   |
| Saving 1,000 edited documents to Postgres | 20 ms    |
| Memory per idle connection                | 17 KiB   |
| Memory per loaded 50-paragraph document   | 132 KiB  |
| 10,000 paragraphs, Markdown to HTML       | 14 ms    |

A loaded document costs about 2.4 KiB per block, plus 10 KiB. Documents stay in memory only while someone is connected, so memory follows concurrent documents, not total ones.
