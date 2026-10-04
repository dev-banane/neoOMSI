import { Failure, PageHead, Tag, Untrusted } from '../components/ui';
import { DISCORD, PLATFORMS, REPO, SERVERS } from '../content/data';
import {
	ago,
	date,
	gh,
	megabytes,
	version,
	type Asset,
	type Release
} from '../lib/github';
import { useAsync } from '../lib/hooks';
import { url } from '../lib/routes';
import { PlatformIcon } from '../components/icons';

function Chip({ asset }: { asset: Asset }) {
	const build = [...SERVERS, ...PLATFORMS].find((b) =>
		asset.name.endsWith(`-${b.key}.${b.ext || 'zip'}`)
	);
	return (
		<li>
			<a className="chip" href={asset.browser_download_url}>
				<PlatformIcon build={build?.key} size={17} />
				<span>{build?.name ?? asset.name}</span>
				<span className="text-muted">{megabytes(asset.size)}</span>
			</a>
		</li>
	);
}

function Entry({ release: r, index }: { release: Release; index: number }) {
	return (
		<li className={index === 0 ? 'latest' : ''}>
			<p className="when">
				{date(r.published_at)}
				<span>{ago(r.published_at)}</span>
			</p>
			<div className="stop">
				<details className="group" open={index < 2}>
					<summary className="flex cursor-pointer list-none flex-wrap items-baseline gap-x-3 gap-y-1 [&::-webkit-details-marker]:hidden">
						<span className="text-[1.6rem] leading-tight font-semibold text-heading">
							{version(r)}
						</span>
						{index === 0 && !r.prerelease && (
							<Tag color="#2da44e">Latest</Tag>
						)}
						{r.prerelease && <Tag color="#d8a020">Pre-release</Tag>}
						<span className="text-muted">
							{r.name && r.name !== r.tag_name ? r.name : ''}
						</span>
						<span className="ml-auto text-[15px] text-muted group-open:hidden">
							Show notes
						</span>
					</summary>
					<div className="pt-4">
						<Untrusted
							className="doc doc-plain max-w-none"
							text={r.body}
							empty="No notes."
						/>
						{r.assets.length > 0 && (
							<ul className="mt-5 flex flex-wrap gap-2">
								{r.assets.map((a) => (
									<Chip key={a.name} asset={a} />
								))}
							</ul>
						)}
					</div>
				</details>
			</div>
		</li>
	);
}

export function Releases() {
	const { data: list, error } = useAsync(
		() => gh<Release[]>('releases?per_page=30'),
		[]
	);

	return (
		<>
			<PageHead>
				<h1 className="display">Releases</h1>
				<p className="mt-6 max-w-[34em] text-[19px] text-muted">
					Every version and what changed, newest first. The newest one
					is also on the{' '}
					<a className="link" href={url('/download/')}>
						download page
					</a>
					.
				</p>
			</PageHead>
			<div className="wrap pb-20 sm:pb-24">
				{error ? (
					<Failure error={error} path="releases" what="releases" />
				) : !list ? (
					<p className="text-muted">Loading releases…</p>
				) : list.length ? (
					<ol className="timeline">
						{list.map((r, i) => (
							<Entry key={r.tag_name} release={r} index={i} />
						))}
					</ol>
				) : (
					<>
						<p className="max-w-[34em] text-[18px]">
							No release has been published yet.
						</p>
						<p className="mt-2 max-w-[34em] text-muted">
							Follow along on{' '}
							<a
								className="link"
								href={`https://github.com/${REPO}`}
							>
								GitHub
							</a>{' '}
							or{' '}
							<a className="link" href={DISCORD}>
								Discord
							</a>{' '}
							to hear when the first one is out.
						</p>
					</>
				)}
			</div>
		</>
	);
}
