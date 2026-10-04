import type { ReactNode } from 'react';
import { PageHead } from '../components/ui';
import { OPENOMSI, REPO } from '../content/data';
import { OPENOMSI_FAQ } from '../content/faq';
import { Icon } from '../components/icons';
import { docPath, url } from '../lib/routes';
import { Answers } from './Faq';

const ROWS: [string, ReactNode, ReactNode][] = [
	[
		'Origin',
		'Fork of openOMSI, developed independently',
		'The original project'
	],
	['License', 'GPL-3.0-or-later (inherited openOMSI code stays MIT)', 'MIT'],
	['Written in', 'Rust', 'Rust'],
	[
		'Graphics',
		'wgpu: DirectX 12, Metal, Vulkan',
		'wgpu: DirectX 12, Metal, Vulkan'
	],
	[
		'Needs OMSI 2',
		'Yes, plays your installed copy',
		'Yes, plays your installed copy'
	],
	[
		'Platforms',
		'Windows, macOS, Linux, Android',
		'Windows, macOS, Linux, Android'
	],
	['Dedicated server', 'Yes, Windows and Linux', 'Yes'],
	[
		'Compatibility target',
		'OMSI 2.2.032, verified subsystem by subsystem',
		'Full compatibility with existing OMSI 2 content'
	],
	['Status', 'Early release', 'Early release']
];

const FOCUS: [string, string, ReactNode][] = [
	[
		'check_circle',
		'Verified behavior',
		<>
			Every subsystem is compared with OMSI&nbsp;2.2.032 and only counts
			as done once it matches and is covered by regression tests. See{' '}
			<a className="link" href={url(docPath('COMPATIBILITY'))}>
				Compatibility
			</a>
			.
		</>
	],
	[
		'public',
		'Clean-room code',
		'No proprietary source code, decompiled binaries or copyrighted assets. neoOMSI only reads the files of your own OMSI 2.'
	],
	[
		'flag',
		'Documented triage',
		<>
			Bugs are classified and verified against the original game before
			they are fixed. See{' '}
			<a className="link" href={url(docPath('ISSUE_TRIAGE'))}>
				Issue triage
			</a>
			.
		</>
	],
	[
		'autorenew',
		'A build for every change',
		<>
			Every change to the main branch is built for all platforms and
			published. See{' '}
			<a className="link" href={url(docPath('RELEASING'))}>
				Releasing &amp; versioning
			</a>
			.
		</>
	]
];

const STEPS: ReactNode[] = [
	<>
		Download neoOMSI for your system from the{' '}
		<a className="link" href={url('/download/')}>
			download page
		</a>{' '}
		and unpack it into its own folder.
	</>,
	'Start neoOMSI and point the launcher to the same OMSI 2 folder you used with openOMSI.',
	<>
		Copy your add-ons into the <code>Mods</code> folder next to neoOMSI, or
		add them on the launcher's <b className="text-ink">Mods</b> page.
	</>,
	'Pick a map, a bus and a duty, and drive. Your OMSI 2 files stay unchanged, so openOMSI keeps working too.'
];

export function OpenOmsi() {
	return (
		<>
			<PageHead>
				<p className="mb-4 text-[15px] font-semibold text-accent">
					Comparison
				</p>
				<h1 className="display">neoOMSI vs openOMSI</h1>
				<p className="mt-6 max-w-[36em] text-[19px] text-muted">
					neoOMSI started as a fork of openOMSI, the Rust recreation
					of OMSI&nbsp;2. Both play the maps, buses and mods of your
					OMSI&nbsp;2 installation. This page shows how they differ
					and how to switch.
				</p>
				<div className="mt-8 flex flex-wrap gap-3">
					<a className="btn gap-2" href={url('/download/')}>
						<Icon name="download" size={20} />
						Download neoOMSI
					</a>
					<a className="btn-quiet gap-2" href={url('/faq/')}>
						<Icon name="info" size={20} />
						Read the FAQ
					</a>
				</div>
			</PageHead>

			<div className="wrap space-y-24 pb-20 sm:pb-24">
				<section>
					<h2 className="section-title">At a glance</h2>
					<div className="mt-8 overflow-x-auto rounded-xl border border-line">
						<table className="w-full border-collapse text-left text-[16px]">
							<thead className="bg-sunken text-[14px] text-muted">
								<tr>
									<th
										scope="col"
										className="px-5 py-3 font-semibold"
									>
										<span className="sr-only">Feature</span>
									</th>
									<th
										scope="col"
										className="px-5 py-3 font-semibold text-heading"
									>
										neoOMSI
									</th>
									<th
										scope="col"
										className="px-5 py-3 font-semibold"
									>
										openOMSI
									</th>
								</tr>
							</thead>
							<tbody>
								{ROWS.map(([label, neo, open]) => (
									<tr
										key={label}
										className="border-t border-line align-top"
									>
										<th
											scope="row"
											className="px-5 py-3 font-medium text-muted"
										>
											{label}
										</th>
										<td className="px-5 py-3 text-heading">
											{neo}
										</td>
										<td className="px-5 py-3">{open}</td>
									</tr>
								))}
							</tbody>
						</table>
					</div>
					<p className="mt-4 text-[15px] text-muted">
						openOMSI details are taken from its own{' '}
						<a className="link" href={OPENOMSI.repo}>
							GitHub repository
						</a>
						.
					</p>
				</section>

				<section className="grid gap-x-16 gap-y-6 lg:grid-cols-[minmax(0,1fr)_minmax(0,2fr)]">
					<h2 className="section-title">Where neoOMSI comes from</h2>
					<div className="max-w-[40em] space-y-4">
						<p>
							neoOMSI was originally forked from openOMSI by
							usonskyyyy. The code it inherited stays under the
							MIT license and is credited in the{' '}
							<a
								className="link"
								href={`https://github.com/${REPO}/blob/main/NOTICE`}
							>
								NOTICE
							</a>{' '}
							file.
						</p>
						<p>
							Since then neoOMSI has been its own project, with
							its own releases, issue tracker and documentation.
							Everything it adds is licensed under
							GPL-3.0-or-later, which requires anyone who
							distributes a modified version to publish its source
							code too.
						</p>
					</div>
				</section>

				<section>
					<h2 className="section-title">What neoOMSI focuses on</h2>
					<div className="mt-8 grid gap-3 sm:grid-cols-2">
						{FOCUS.map(([symbol, title, text]) => (
							<div
								key={title}
								className="flex flex-col gap-3 rounded-xl bg-[color-mix(in_srgb,var(--page)_50%,var(--raised))] p-5"
							>
								<span className="text-accent">
									<Icon name={symbol} size={28} />
								</span>
								<div>
									<h3 className="text-[1.1rem]">{title}</h3>
									<p className="mt-1 text-[16px] text-muted">
										{text}
									</p>
								</div>
							</div>
						))}
					</div>
				</section>

				<section className="grid gap-x-16 gap-y-10 lg:grid-cols-[1fr_2fr]">
					<h2 className="section-title">Switching from openOMSI</h2>
					<ol className="route max-w-[38rem]">
						{STEPS.map((step, i) => (
							<li key={i}>
								<p className="text-muted">{step}</p>
							</li>
						))}
					</ol>
				</section>

				<section>
					<h2 className="section-title">
						Questions about neoOMSI and openOMSI
					</h2>
					<div className="mt-6">
						<Answers list={OPENOMSI_FAQ} level={3} />
					</div>
				</section>
			</div>
		</>
	);
}
