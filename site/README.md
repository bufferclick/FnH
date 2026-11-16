# fnh.is-cool.dev

The site is these three files and the icon: `index.html`, `styles.css`, `assets/icon.png`. No build
step, no framework, no JavaScript beyond the one request that fills the download button in.

## Publishing it

The site is served from the `fnh` repository's GitHub Pages, with `CNAME` holding
`fnh.is-cool.dev`:

1. **Settings → Pages → Source: Deploy from a branch**, branch `main`, folder `/site`. (Or use
   *GitHub Actions* and publish this folder — `CNAME` must reach the deployed output either way.)
2. In the domain's DNS, point `fnh.is-cool.dev` at GitHub Pages:
   `CNAME  fnh  bufferclick.github.io` — or, for the apex, the four `A` records GitHub lists under
   *Settings → Pages → Custom domain*.
3. **Settings → Pages → Custom domain: `fnh.is-cool.dev`**, then *Enforce HTTPS* once the
   certificate is issued.

`https://fnh.is-cool.dev/` then serves `index.html` directly. The `CNAME` file is what keeps the
domain on redeploy, so leave it in place.

## The download button

`index.html` points the buttons at `github.com/bufferclick/fnh/releases/latest` in the markup, and
the small script asks GitHub's API for that release's `fnh-*.zip` and points them straight at the
file, with its name and size in the fine print. If the API is rate-limited or offline, the buttons
stay on the releases page, which is a working download too.

Nothing else to update when a version ships — `.github/workflows/fh.yml` publishes the release and
this page follows it.

## Editing it

Keep it plain: no taglines, no gradients, no invented claims. The accent colour is the app's own
highlight (`#96db59`), used only for the primary button. Check both light and dark
(`prefers-color-scheme`) and a narrow window (the layout is one column below 40rem).
