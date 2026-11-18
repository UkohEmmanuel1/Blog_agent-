# ATLAS Web

Next.js App Router frontend for the ATLAS editorial platform.

## Local development
1. Copy .env.example to .env.local.
2. Set the public Supabase project URL and anon/publishable key.
3. Set NEXT_PUBLIC_ATLAS_API_URL to the Rust API URL (default http://localhost:8080/api/v1).
4. Start the Rust API and apply database migrations.
5. Run npm install and npm run dev from this directory. Open http://localhost:3000.

## Production
Run npm install, npm run typecheck and npm run build. Deploy this directory as a Next.js application.

Only the Supabase anon/publishable key belongs in NEXT_PUBLIC variables. Never expose service-role keys, database credentials, Redis URLs, S3 secrets, LLM API keys or social access tokens to the browser.

The browser authenticates with Supabase Auth and sends the access token to the Rust API as a bearer token. The API remains responsible for authorization and role enforcement.