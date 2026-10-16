# Flotilla marketing site

Static Vite site with a small Cloudflare Worker route that resolves the latest
macOS release from Flotilla's updater manifest.

```sh
pnpm install
pnpm dev
pnpm check
```

## Cloudflare

`wrangler.jsonc` serves the built files through Workers Static Assets on your
account's `workers.dev` domain. It does not require the Flotilla domain or account.
The Worker names in `wrangler.jsonc` and `wrangler.updater-bridge.jsonc` still say
`gravity` on purpose: renaming them deploys new Workers, so they change only when
the deployment moves. The canonical URLs still point at `getgravity.build` until a
Flotilla domain is chosen.

```sh
pnpm worker:dev
pnpm deploy
```

Deployment requires your own Cloudflare account and credentials accepted by
Wrangler. Set `vars.RELEASE_MANIFEST_URL` to your HTTPS updater manifest to enable
`/download/latest`; it returns 503 without configuration. Add a `routes` entry
with your custom domain if desired. Review the canonical URLs, sitemap, and
branding when forking.

## Legacy updater bridge

`wrangler.updater-bridge.jsonc` builds a separate, dependency-free compatibility
Worker. It serves only GET/HEAD `/desktop/flotilla/latest.json` and its legacy
alias `/desktop/gravity/latest.json`, returning the configured upstream manifest
without a redirect or content changes. Both paths answer identically; keep the
legacy alias, because apps installed before the rename keep polling it. Other paths
and methods are rejected. Upstream errors/invalid manifests return 502; responses
are not cached. The bridge does not forward caller headers or query parameters.

The default upstream is empty and there are no production routes in source.
Keep deployment configuration specific to your distribution. From this directory:

```sh
pnpm exec wrangler deploy --config wrangler.updater-bridge.jsonc --dry-run
pnpm exec wrangler deploy --config wrangler.updater-bridge.jsonc \
  --var "RELEASE_MANIFEST_URL:https://github.com/OWNER/REPO/releases/latest/download/latest.json"
```

Verify the Worker on its `workers.dev` URL first. Publish and verify the transition
release's GitHub assets, manifest and updater signature before attaching the exact
manifest routes. Do not replace the R2 custom domain or route the entire
host: existing versioned downloads must continue reaching their original objects.
Attach one route per path, each with a trailing `*` to include requests with query
parameters; the handler still rejects every pathname except the two manifest paths:

```sh
pnpm exec wrangler deploy --config wrangler.updater-bridge.jsonc \
  --var "RELEASE_MANIFEST_URL:https://github.com/OWNER/REPO/releases/latest/download/latest.json" \
  --route "downloads.example.com/desktop/flotilla/latest.json*" \
  --route "downloads.example.com/desktop/gravity/latest.json*"
```

Set the marketing Worker's runtime **and automatic-build** `RELEASE_MANIFEST_URL`
to the GitHub latest manifest. Preserve the explicit `--var` deploy argument and
the empty committed default. Test an existing updater's check/download/signature
verification through the legacy URL, and the transition client's GitHub endpoint.
Keep the original signing keys and the bridge for users who upgrade much later.

For rollback, repoint the bridge to a verified version-specific GitHub manifest,
or remove only its route to expose the retained R2 manifest again. Keep a private
copy of the prior route/settings/manifest before activation. Removing a route or
repointing a manifest affects future checks; it does not downgrade installed apps.
Never replace a published version's signed archives to roll back.
