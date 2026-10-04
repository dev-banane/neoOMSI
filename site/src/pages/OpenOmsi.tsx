import type { ReactNode } from 'react';
import { PageHead } from '../components/ui';
import { OPENOMSI, REPO } from '../content/data';
import { OPENOMSI_FAQ } from '../content/faq';
import { Icon } from '../components/icons';
import { docPath, url } from '../lib/routes';
import { Answers } from './Faq';

const ROWS: [string, ReactNode, ReactNode][] = [
	['Needs OMSI 2', 'Yes, uses your own copy', 'Yes, uses your own copy'],
	['Runs on', 'Windows, Mac, Linux, Android', 'Windows, Mac, Linux, Android'],
	['Multiplayer server', 'Yes, Windows and Linux', 'Yes'],
	[
		'Goal',
		'Play exactly like OMSI 2',
		'Full compatibility with existing OMSI 2 content'
	],
	['Free and open source', 'Yes', 'Yes'],
	['Status', 'Early release', 'Early release']
];

const FOCUS: [string, string, ReactNode][] = [
	[
		'check_circle',
		'More stable',
		'Two team members check every change before it goes in, and automatic tests make sure fixed bugs stay fixed.'
	],
	[
		'sync_alt',
		'Plays like the original',
		<>
			Each part of the game is checked against OMSI&nbsp;2 until it
			behaves the same. See{' '}
			<a className="link" href={url(docPath('COMPATIBILITY'))}>
				Compatibility
			</a>
			.
		</>
	],
	[
		'public',
		'Nothing copied from OMSI 2',
		'It only reads the files of the copy you own.'
	],
	[
		'autorenew',
		'Tested releases',
		<>
			Changes are bundled into proper releases that are tested before
			they come out. Want the newest fixes sooner? Try the nightly
			build. See{' '}
			<a className="link" href={url('/releases/')}>
				Releases
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
					neoOMSI and openOMSI are two different projects. Both let
					you play OMSI&nbsp;2 with your own maps, buses and mods.
					Here is how they differ, and how to switch.
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
					<h2 className="section-title">How the projects relate</h2>
					<div className="max-w-[40em] space-y-4">
						<p>
							neoOMSI has its own team, its own releases and its
							own place to report bugs. It is not connected to
							openOMSI.
						</p>
						<p>
							Some of neoOMSI's early code came from openOMSI by
							usonskyyyy, and it is credited in the{' '}
							<a
								className="link"
								href={`https://github.com/${REPO}/blob/main/NOTICE`}
							>
								NOTICE
							</a>{' '}
							file.
						</p>
					</div>
				</section>

				<section>
					<h2 className="section-title">What neoOMSI focuses on</h2>
					<div className="mt-8 grid gap-3 sm:grid-cols-2">
						{FOCUS.map(([symbol, title, text]) => (
							<div
								key={title}
								className="card flex flex-col gap-3 p-5"
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
