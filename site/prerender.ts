import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { pathToFileURL } from 'node:url'
import sharp from 'sharp'

const dist = join(import.meta.dirname, 'dist')
const bundle = join(import.meta.dirname, 'dist-server', 'server.js')
const s: typeof import('./src/server') = await import(pathToFileURL(bundle).href)

const template = readFileSync(join(dist, 'index.html'), 'utf8')

const write = (file: string, content: string | Buffer) => {
  mkdirSync(dirname(join(dist, file)), { recursive: true })
  writeFileSync(join(dist, file), content)
}

const manifest = `<link rel="manifest" href="${s.url('/site.webmanifest')}">`

const page = (path: string, body: string) =>
  template
    .replace('</head>', `${s.headTags(s.meta(path))}\n${manifest}\n</head>`)
    .replace('<div id="root"></div>', `<div id="root">${body}</div>`)

for (const path of s.PAGES) write(path === '/' ? 'index.html' : `${path.slice(1)}/index.html`, page(path, s.render(path)))

write('404.html', page('/404', ''))

const canonical = (path: string) => s.absolute(path === '/' ? '' : `${path}/`)

write(
  'sitemap.xml',
  `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${s.PAGES.map((p) => `  <url><loc>${canonical(p)}</loc></url>`).join('\n')}
</urlset>
`,
)

write('robots.txt', `User-agent: *\nAllow: /\n\nSitemap: ${s.absolute('sitemap.xml')}\n`)

const app = (file: string) => join(import.meta.dirname, '../assets/icons/app', file)
write('favicon.ico', readFileSync(app('neoomsi.ico')))
write('icon-192.png', await sharp(app('neoomsi-512.png')).resize(192).png().toBuffer())
write('icon-512.png', readFileSync(app('neoomsi-512.png')))
// iOS fills transparent touch icons with black, so the page background is baked in.
write('apple-touch-icon.png', await sharp(app('neoomsi-512.png')).resize(180).flatten({ background: '#0f0f0f' }).png().toBuffer())

write(
  'site.webmanifest',
  JSON.stringify(
    {
      name: 'neoOMSI',
      short_name: 'neoOMSI',
      description: s.DESCRIPTION,
      start_url: s.url('/'),
      scope: s.url('/'),
      display: 'standalone',
      background_color: '#0f0f0f',
      theme_color: '#0f0f0f',
      icons: [
        { src: s.url('/icon-192.png'), sizes: '192x192', type: 'image/png' },
        { src: s.url('/icon-512.png'), sizes: '512x512', type: 'image/png' },
      ],
    },
    null,
    2,
  ),
)

write(
  '.well-known/security.txt',
  `Contact: https://github.com/${s.REPO}/security/advisories/new
Expires: ${new Date(Date.now() + 365 * 86_400_000).toISOString()}
Preferred-Languages: en, de
Canonical: ${s.absolute('.well-known/security.txt')}
`,
)

write(
  '_headers',
  `/*
  X-Content-Type-Options: nosniff
  Referrer-Policy: strict-origin-when-cross-origin
  X-Frame-Options: DENY
  Permissions-Policy: camera=(), microphone=(), geolocation=()

/assets/*
  Cache-Control: public, max-age=31536000, immutable
`,
)

const docs = s.DOCS.map((d) => {
  const doc = s.renderDoc(d.file)!
  return `- [${doc.title}](${canonical(`/docs/${d.slug}`)}): ${s.plain(doc.lead) || d.title}`
})

const summary = `# neoOMSI

> ${s.DESCRIPTION}

neoOMSI needs an installed copy of OMSI 2 and contains no game content of its own. It targets the observable behavior of OMSI 2.2.032 and is licensed under GPL-3.0-or-later. It started as a fork of openOMSI and is developed independently.
`

write(
  'llms.txt',
  `${summary}
## Pages

- [Download](${canonical('/download')}): Builds for Windows, macOS, Linux, Android and the dedicated server
- [FAQ](${canonical('/faq')}): What neoOMSI is, what it needs and which platforms it runs on
- [neoOMSI vs openOMSI](${canonical('/openomsi')}): How neoOMSI relates to openOMSI and how to switch
- [Releases](${canonical('/releases')}): Every version and what changed

## Docs

${docs.join('\n')}

## Optional

- [Full text](${s.absolute('llms-full.txt')}): The FAQ and every document in one file
- [Source code](https://github.com/neoOMSI/neoOMSI): The neoOMSI repository on GitHub
`,
)

const answers = (list: { q: string; a: string }[]) => list.map(({ q, a }) => `### ${q}\n\n${a}`).join('\n\n')

write(
  'llms-full.txt',
  `${summary}
## Frequently asked questions

${answers(s.FAQ)}

## neoOMSI and openOMSI

${answers(s.OPENOMSI_FAQ)}

${s.DOCS.map((d) => s.source(d.file)!.trim().replace(/^#/gm, '##')).join('\n\n')}
`,
)

const { width, height } = s.OG_IMAGE
const wordmark = await sharp(join(import.meta.dirname, '../assets/logos/neoOMSI-wordmark.png')).trim().resize({ width: 720 }).png().toBuffer()
const logo = await sharp(wordmark).metadata()
const road = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}">
  <rect width="100%" height="100%" fill="#0f0f0f"/>
  <g stroke="#fd6b00" stroke-linecap="round">
    <path d="M-40 560 L1240 470" stroke-width="7"/>
    <path d="M-40 640 L1240 520" stroke-width="7"/>
    <path d="M-40 600 L1240 495" stroke-width="10" stroke-dasharray="90 150"/>
  </g>
</svg>`
write(
  s.OG_IMAGE.path,
  await sharp(Buffer.from(road))
    .composite([{ input: wordmark, left: Math.round((width - logo.width!) / 2), top: Math.round(height * 0.42 - logo.height! / 2) }])
    .png()
    .toBuffer(),
)

rmSync(join(import.meta.dirname, 'dist-server'), { recursive: true, force: true })
console.log(`Prerendered ${s.PAGES.length} pages`)
