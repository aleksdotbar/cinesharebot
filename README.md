# Cinesharebot

Rust Telegram bot for searching and sharing movie and TV posters through inline queries. Axum serves `POST /telegram/webhook` and unauthenticated `GET /healthz`.

## Runtime configuration

Required environment variables:

- `BOT_TOKEN`: Telegram bot token.
- `TMDB_KEY`: TMDB bearer credential.
- `ANALYTICS_CHAT_ID`: existing destination for best-effort analytics/error messages.
- `WEBHOOK_SECRET`: 1–256 ASCII letters, digits, underscores or hyphens. Missing, empty or invalid values prevent startup. Every webhook request must supply the matching `X-Telegram-Bot-Api-Secret-Token` header; authentication runs before JSON parsing.

`WEBHOOK_BASE_URL` is optional. When supplied, startup registers `<base-url>/telegram/webhook` with the secret and preserves pending updates. Startup always calls Telegram `getMe` before opening its listener. Running the binary with real credentials can therefore contact Telegram and change the real webhook; builds and unit tests do not run that startup path.

`HOST` defaults to `0.0.0.0`, `PORT` to `3000`. These are container-private listeners in the infrastructure setup. Cloudflare Tunnel supplies public HTTPS. The service needs DNS and outbound HTTPS to Telegram and TMDB; no database or persistent application volume is currently used.

Keep credentials out of Git, Nix inputs/store paths, command arguments and logs. Production supplies the runtime environment through SOPS and native systemd credential delivery; local `.env` files remain ignored. Configuration debug output redacts credentials; Telegram request errors strip credential-bearing URLs, and invalid-response errors omit raw response bodies.

## Local checks

```sh
cargo test --locked
cargo clippy --locked --all-targets
```

Run one heavy check at a time with one Cargo build job. On the shared devbox, use a systemd scope capped at 6 GiB RAM and 200% CPU; keep build logs outside temporary directories if they must survive a restart. No real Telegram/TMDB credentials are needed for unit tests.

Existing request concurrency, upstream response buffering and provider `Retry-After` waits have not received workload acceptance testing. Container resource limits are safeguards, not a throughput or overload guarantee. Real-host, Cloudflare/DNS/HTTPS and Telegram acceptance remain separate authorized operations.

Sources: [Telegram webhook contract](https://core.telegram.org/bots/api#setwebhook), [reqwest 0.12.28 error URL handling](https://github.com/seanmonstar/reqwest/blob/v0.12.28/src/error.rs).
