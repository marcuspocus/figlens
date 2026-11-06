# Changelog

## [0.14.1](https://github.com/runkids/figlens/compare/v0.14.0...v0.14.1) (2026-11-06)


* **release:** request 0.14.1 ([0e1ed6d](https://github.com/runkids/figlens/commit/0e1ed6d4890b72bdbb0d8015b6dddaf711528d5d))


### Bug Fixes

* **pull:** verify headed login and improve first-run setup ([#248](https://github.com/runkids/figlens/issues/248)) ([ad3ffb6](https://github.com/runkids/figlens/commit/ad3ffb66b427658cef739a5d2121ff60c86b6cf5))

## [0.14.0](https://github.com/runkids/figlens/compare/v0.13.1...v0.14.0) (2026-11-06)


### New Features

* **freshness:** support named Figma API accounts ([#245](https://github.com/runkids/figlens/issues/245)) ([9e12203](https://github.com/runkids/figlens/commit/9e12203014f5a4dc6c400ee969710905882e9c71))
* **nodes:** write MCP nodes JSON to a file with out ([#242](https://github.com/runkids/figlens/issues/242)) ([4151b4b](https://github.com/runkids/figlens/commit/4151b4b2e4dda2b1edb1910f39a8380c20c205fd))
* **pull:** support named Figma browser accounts ([#243](https://github.com/runkids/figlens/issues/243)) ([b872700](https://github.com/runkids/figlens/commit/b8727001a6137099882e50b65ff184628af779c1))

## [0.13.1](https://github.com/runkids/figlens/compare/v0.13.0...v0.13.1) (2026-11-05)


* **release:** request 0.13.1 ([f932055](https://github.com/runkids/figlens/commit/f93205512d0fe2df6a8629759aa6f47b0ef079cb))


### New Features

* **herdr:** hide the side panel from the panel's own header ([a8b97ed](https://github.com/runkids/figlens/commit/a8b97edbdb99c6227c47b50857d434505523ef2d))
* **pull:** sign out with --logout and drive agent-browser as an alternative browser ([#239](https://github.com/runkids/figlens/issues/239)) ([9f7f45f](https://github.com/runkids/figlens/commit/9f7f45ffdd5af5903e0f364a248576dbc06db300))


### Bug Fixes

* **herdr:** stop the panel freezing Herdr when several agents work on one design ([#240](https://github.com/runkids/figlens/issues/240)) ([b73c2ed](https://github.com/runkids/figlens/commit/b73c2edcf8d56be73f4484af34ea6e091f6b1534))

## [0.13.0](https://github.com/runkids/figlens/compare/v0.12.1...v0.13.0) (2026-11-04)


### New Features

* **freshness:** tell agents when a saved .fig is older than its online file ([#230](https://github.com/runkids/figlens/issues/230)) ([09bd9d5](https://github.com/runkids/figlens/commit/09bd9d5ee1ba9fe588fbcc8427778e94f84c0556))
* **herdr:** add a figlens panel and layout for Herdr ([#229](https://github.com/runkids/figlens/issues/229)) ([4bcba8e](https://github.com/runkids/figlens/commit/4bcba8e7bfe9726229149420c41ed5573dd1b90a))
* **herdr:** show what agents did with each screen on a board ([#236](https://github.com/runkids/figlens/issues/236)) ([a73ca51](https://github.com/runkids/figlens/commit/a73ca5162178e7c756c42985c5b0c25bcdf65857))
* **inspect:** report the node's ancestor path ([#228](https://github.com/runkids/figlens/issues/228)) ([c341c4f](https://github.com/runkids/figlens/commit/c341c4f68ecc99761de972608ff97f96687c6943))
* **mcp:** journal agent reads in Herdr and show them in the panel ([#231](https://github.com/runkids/figlens/issues/231)) ([59bb2d6](https://github.com/runkids/figlens/commit/59bb2d67fdb56d3bdf062946e4196c2a008ac385))
* **preview:** stop a viewer after it idles ([#234](https://github.com/runkids/figlens/issues/234)) ([8d5df0a](https://github.com/runkids/figlens/commit/8d5df0ac1f67919795013308497934bf8e4a7ce9))
* **pull:** refresh mapped .fig copies through a browser sign-in ([#232](https://github.com/runkids/figlens/issues/232)) ([cc7c885](https://github.com/runkids/figlens/commit/cc7c88507c2db9c411f79c02dc597060afe90d22))


### Bug Fixes

* **herdr:** keep the figlens panel when switching spaces ([#233](https://github.com/runkids/figlens/issues/233)) ([d7224f5](https://github.com/runkids/figlens/commit/d7224f5217cdc2dd63f03ff7d58e50b777abb474))
* **herdr:** name figlens's workflow tool when handing work to Claude Code ([#235](https://github.com/runkids/figlens/issues/235)) ([bee2897](https://github.com/runkids/figlens/commit/bee28974724f6e9854f9a0ad2b27d527661edd2a))
* **preview:** keep the header status pill readable at every width ([effa208](https://github.com/runkids/figlens/commit/effa20867d9f63f875f6fdcb53f81b958f219ad8))

## [0.12.1](https://github.com/runkids/figlens/compare/v0.12.0...v0.12.1) (2026-11-04)


* **release:** request 0.12.1 ([c9b2ec3](https://github.com/runkids/figlens/commit/c9b2ec35184b7178aa75d473535c0847a718152c))


### New Features

* **clipboard:** read the system clipboard ([#220](https://github.com/runkids/figlens/issues/220)) ([10de609](https://github.com/runkids/figlens/commit/10de609da556586b936bc3800d133c50656ebc51))
* **cli:** print only a command's usage for &lt;command&gt; --help ([#221](https://github.com/runkids/figlens/issues/221)) ([353c71f](https://github.com/runkids/figlens/commit/353c71fedf3d86459c5f6e3a6ac31a5f70388916))

## [0.12.0](https://github.com/runkids/figlens/compare/v0.11.4...v0.12.0) (2026-11-03)


### New Features

* **clipboard:** read a selection copied from Figma without a local copy ([#212](https://github.com/runkids/figlens/issues/212)) ([1ad0263](https://github.com/runkids/figlens/commit/1ad026315ac616efefc84945ed9c3532d02d47ac))
* **nodes:** emit Dev Mode annotations and Ready for dev status ([#213](https://github.com/runkids/figlens/issues/213)) ([7801d93](https://github.com/runkids/figlens/commit/7801d93d6c997a1aa339e65366c571866fe31edf))


### Bug Fixes

* **export:** warn when an image or component a .fig lacks is drawn empty ([#212](https://github.com/runkids/figlens/issues/212)) ([1ad0263](https://github.com/runkids/figlens/commit/1ad026315ac616efefc84945ed9c3532d02d47ac))

## [0.11.4](https://github.com/runkids/figlens/compare/v0.11.3...v0.11.4) (2026-11-02)


### ⚠ BREAKING CHANGES

* **nodes:** omit absent semantics and state verification once ([#202](https://github.com/runkids/figlens/issues/202))

* **release:** request 0.11.4 ([0f6013d](https://github.com/runkids/figlens/commit/0f6013d30fb54176352161541010c15da7cb2cd2))
* **release:** request 0.12.0 ([47676c8](https://github.com/runkids/figlens/commit/47676c84a7dcf4f02b7be0559c563907e3fdff71))


### New Features

* **nodes:** add opt-in compact style and subtree sharing ([#211](https://github.com/runkids/figlens/issues/211)) ([217e8c0](https://github.com/runkids/figlens/commit/217e8c015fbb0a1c6fb9a845178eb9637384679b))
* **nodes:** emit grid auto layout counts, gaps and track sizes ([#209](https://github.com/runkids/figlens/issues/209)) ([159500a](https://github.com/runkids/figlens/commit/159500ae90afa2c012dad01c0eac01f0f728bf99))
* **nodes:** report stored Motion and code-layer content as handoff gaps ([#210](https://github.com/runkids/figlens/issues/210)) ([c6251bf](https://github.com/runkids/figlens/commit/c6251bfe156d7224b9520486c3603c7479232ace))


### Bug Fixes

* **render:** report paint types the renderer does not draw ([#201](https://github.com/runkids/figlens/issues/201)) ([21ba340](https://github.com/runkids/figlens/commit/21ba340f496d8b63c2e9a868506f6825cc671909))


### Performance

* **nodes:** omit absent semantics and state verification once ([#202](https://github.com/runkids/figlens/issues/202)) ([32c48da](https://github.com/runkids/figlens/commit/32c48dac17a66425b9b1dc97d78219e35eff70a8))

## [0.11.3](https://github.com/runkids/figlens/compare/v0.11.2...v0.11.3) (2026-11-02)


* **release:** request 0.11.3 ([240b1bb](https://github.com/runkids/figlens/commit/240b1bb3afc5e5668e2a77497ba15174fde41fe9))


### New Features

* **bundle:** record the source .fig's sha256 with --hash-source ([#199](https://github.com/runkids/figlens/issues/199)) ([275fc03](https://github.com/runkids/figlens/commit/275fc037f2e17fd05da12cd5fcf1471494bb7a2d))

## [0.11.2](https://github.com/runkids/figlens/compare/v0.11.1...v0.11.2) (2026-11-01)


### Bug Fixes

* **workflow:** ask implementing agents for real frontend code ([#197](https://github.com/runkids/figlens/issues/197)) ([f3607b4](https://github.com/runkids/figlens/commit/f3607b4a092015efe0ff8daf2525e0aa184be0c4))

## [0.11.1](https://github.com/runkids/figlens/compare/v0.11.0...v0.11.1) (2026-11-01)


### New Features

* **compare-image:** compare a full-page capture shorter than the frame ([#191](https://github.com/runkids/figlens/issues/191)) ([dde88e6](https://github.com/runkids/figlens/commit/dde88e6eaef29e9c1fccdbb74414adfb0c5cf457))
* **compare-image:** mask data-driven layers, tag image regions, opt-in threshold ([#193](https://github.com/runkids/figlens/issues/193)) ([da8fb1b](https://github.com/runkids/figlens/commit/da8fb1b2591e58e042a789004da150ea5f92c1a2))
* **site:** add figlens.runkids.cc with docs generated from the repo ([ad4f4f3](https://github.com/runkids/figlens/commit/ad4f4f3d5fd823d302e6db476961c43087938bda))

## [0.11.0](https://github.com/runkids/figlens/compare/v0.10.1...v0.11.0) (2026-10-31)


### New Features

* **compare-image:** measure a screenshot of the built screen against its frame ([#187](https://github.com/runkids/figlens/issues/187)) ([3a4595d](https://github.com/runkids/figlens/commit/3a4595d1751df3f0ce21668d10bfa91499365108))
* **export:** draw chosen layers hidden with --hide ([#183](https://github.com/runkids/figlens/issues/183)) ([d6e5058](https://github.com/runkids/figlens/commit/d6e50585f9e12a4009ea009cebe03f576507864a))
* **export:** export many nodes in one call with a manifest ([#182](https://github.com/runkids/figlens/issues/182)) ([386a370](https://github.com/runkids/figlens/commit/386a3704b6e1a76bf3791ae95fcdda53e4f7e218))
* **inspect:** add --full-text and --colors for implementing a screen from inspect alone ([#180](https://github.com/runkids/figlens/issues/180)) ([6a1cf38](https://github.com/runkids/figlens/commit/6a1cf38f5f0d45734605f5edcd113da69b7c5185))
* **inspect:** compare any two nodes with --compare ([#186](https://github.com/runkids/figlens/issues/186)) ([deef0da](https://github.com/runkids/figlens/commit/deef0da50a6cd2f6cb6ed4c3a8e53813ae3ebd4f))
* **plugins:** install the skill and MCP server as one host plugin ([#185](https://github.com/runkids/figlens/issues/185)) ([2744e2e](https://github.com/runkids/figlens/commit/2744e2efcfeda1e845fba932453169944fdbf2e1))
* **tokens:** resolve DTCG aliases into other collections ([#178](https://github.com/runkids/figlens/issues/178)) ([f8561eb](https://github.com/runkids/figlens/commit/f8561ebfcff9b73e49d0e14a78efd82e4e8fdf92))
* **verify:** find local font files for design faces and emit [@font-face](https://github.com/font-face) ([#181](https://github.com/runkids/figlens/issues/181)) ([95b4afa](https://github.com/runkids/figlens/commit/95b4afa2ed3754eab29dc55c17f32a02c55d5371))

## [0.10.1](https://github.com/runkids/figlens/compare/v0.10.0...v0.10.1) (2026-10-31)


### New Features

* **inspect:** show alignment, sizing and effects in --flat lines ([#172](https://github.com/runkids/figlens/issues/172)) ([dcc3621](https://github.com/runkids/figlens/commit/dcc3621ac5769f1e88b975b7169e266afc576d51))
* **states:** pair unique same-name variant children and name bound variables ([#171](https://github.com/runkids/figlens/issues/171)) ([aafaece](https://github.com/runkids/figlens/commit/aafaecece99af76d2ede2971fe5003c94ed8b3cf))
* **variables:** export library collections from cached copies and name them in usages ([#170](https://github.com/runkids/figlens/issues/170)) ([973f669](https://github.com/runkids/figlens/commit/973f66916c686d1533518a8876097d889c76b246))
* **workflow:** serve index guidance as its own search topic ([#169](https://github.com/runkids/figlens/issues/169)) ([7cf0077](https://github.com/runkids/figlens/commit/7cf0077485fad80daa18d32e9b7a27bb5de27a7d))


### Bug Fixes

* **diff:** say a stopped comparison stopped instead of "no changes" ([#167](https://github.com/runkids/figlens/issues/167)) ([dd94314](https://github.com/runkids/figlens/commit/dd94314e632d6344751137f19dc4172baee1569e))
* **export:** report render diagnostics in export warnings ([#165](https://github.com/runkids/figlens/issues/165)) ([e6b8ee9](https://github.com/runkids/figlens/commit/e6b8ee98d6cbf5bc2cd5f4d43a6392ef03d2304d))
* **mcp:** name a missing output folder instead of calling it outside the roots ([#168](https://github.com/runkids/figlens/issues/168)) ([9531ee4](https://github.com/runkids/figlens/commit/9531ee438f46b277655b21bbae88cc3115ae51eb))

## [0.10.0](https://github.com/runkids/figlens/compare/v0.9.3...v0.10.0) (2026-10-31)


### New Features

* **bundle:** read the --frames list from stdin with - ([#118](https://github.com/runkids/figlens/issues/118)) ([2bbc95f](https://github.com/runkids/figlens/commit/2bbc95f2f670bfebc63b781995bc53fd0e55eeb6))
* **changes:** compare design revisions and retain opt-in history ([#132](https://github.com/runkids/figlens/issues/132)) ([b6437a5](https://github.com/runkids/figlens/commit/b6437a5f650f6c0a5c794c55a86f0bc3c7d7a0c2))
* **changes:** watch saved designs and report affected scopes ([#137](https://github.com/runkids/figlens/issues/137)) ([890f8da](https://github.com/runkids/figlens/commit/890f8da9519222bd18dd77d95240ca3ef18afb06))
* **index:** add local workspace storage and lifecycle ([#90](https://github.com/runkids/figlens/issues/90) part 1) ([#119](https://github.com/runkids/figlens/issues/119)) ([0b6d8a6](https://github.com/runkids/figlens/commit/0b6d8a6fd42ec1fbd47dcadd3116ba6e0938e94a))
* **index:** durable opt-in workspace search, rebuild and cancellable jobs ([#90](https://github.com/runkids/figlens/issues/90) part 2) ([#127](https://github.com/runkids/figlens/issues/127)) ([d187d2d](https://github.com/runkids/figlens/commit/d187d2d0f3496bf462e43bc4a61b978b968bc0ef))
* **index:** register directories and gs:// objects, lease query cursors ([#139](https://github.com/runkids/figlens/issues/139)) ([e7b7b86](https://github.com/runkids/figlens/commit/e7b7b8680d7cdd6e5573ccde4861f65f17cdf29e))
* **inspect,export:** flat per-node lines and project icon matching ([#114](https://github.com/runkids/figlens/issues/114)) ([e7c5c76](https://github.com/runkids/figlens/commit/e7c5c76962a87a3cea7e143c1e6d22be9871e457))
* **inspect:** mark derived spacing under parents without auto-layout ([#112](https://github.com/runkids/figlens/issues/112)) ([7d6c075](https://github.com/runkids/figlens/commit/7d6c0752d6b5e3f45aa63c8c79ba086e42392bdd))
* **library:** library usages and saved questions across sources ([#93](https://github.com/runkids/figlens/issues/93), first slice) ([#138](https://github.com/runkids/figlens/issues/138)) ([2b85531](https://github.com/runkids/figlens/commit/2b8553165101c5c1ef5c9059fbe71bb98ef00437))
* **preview:** name page, source and snapshot on results and handoff ([#140](https://github.com/runkids/figlens/issues/140)) ([cc6467a](https://github.com/runkids/figlens/commit/cc6467afecae34d72c29c10d2027567cbcb6a4bb))
* **preview:** name the live file and the reference scope's coverage in handoff ([#143](https://github.com/runkids/figlens/issues/143)) ([887d589](https://github.com/runkids/figlens/commit/887d589ac4589cc00fd455f70255585356f39c29))
* **search:** add optional local semantic search and its runtime ([#152](https://github.com/runkids/figlens/issues/152)) ([30a8bcf](https://github.com/runkids/figlens/commit/30a8bcfe00ea4766348b87c28ccc2b1a8a8f5896))
* **search:** add visual and hybrid search across hosts and viewer ([#157](https://github.com/runkids/figlens/issues/157)) ([dbd6e27](https://github.com/runkids/figlens/commit/dbd6e271df488d012435370670ef8457838ce7aa))
* **search:** find effective instance content with bounded ranked retrieval ([#102](https://github.com/runkids/figlens/issues/102)) ([3240c51](https://github.com/runkids/figlens/commit/3240c515a6baffc18252755eec501fc023e61e46))
* **tree:** give top-level nodes their first visible text ([#121](https://github.com/runkids/figlens/issues/121)) ([bf039c8](https://github.com/runkids/figlens/commit/bf039c84077ffb51421f58efed25c498d3a618a8))
* **tree:** list every screen for frame lists with --top-frames ([#116](https://github.com/runkids/figlens/issues/116)) ([dd29b83](https://github.com/runkids/figlens/commit/dd29b832cfd1df5373d0c820407404408815cf84))
* **usages:** expose exact design relationships across hosts ([#129](https://github.com/runkids/figlens/issues/129)) ([bcf2122](https://github.com/runkids/figlens/commit/bcf2122b1c583cd994bb4d99ad5a214763926ba1))
* **variables:** export DTCG tokens from local snapshots ([#147](https://github.com/runkids/figlens/issues/147)) ([c0d23c7](https://github.com/runkids/figlens/commit/c0d23c7e6ecda04cc8848cf43c9ca85078a8f2ad))


### Bug Fixes

* bound layer recursion so deep files fail instead of aborting ([#124](https://github.com/runkids/figlens/issues/124)) ([9198a58](https://github.com/runkids/figlens/commit/9198a5863d8cf1c78e5ae9cecae950eea8e751ce))
* **bundle:** warn about SVG transform and ids in context.md and MCP export ([#106](https://github.com/runkids/figlens/issues/106)) ([5036993](https://github.com/runkids/figlens/commit/503699330cf590a2dcb7992a322d459a02b89968))
* **diff:** count inherited variable modes and keep no-op track request ids ([#136](https://github.com/runkids/figlens/issues/136)) ([b4be0ae](https://github.com/runkids/figlens/commit/b4be0ae86694bcf0e54d04617e006e51ee2ca63d))
* **opencode:** wait for MCP tools to reach the registry before shortcuts ([#126](https://github.com/runkids/figlens/issues/126)) ([b1792ff](https://github.com/runkids/figlens/commit/b1792ffd625e847b9f90346f262feb81ad73bec1))
* **pi:** retry network-level failures while waiting for the viewer ([#115](https://github.com/runkids/figlens/issues/115)) ([dc257f6](https://github.com/runkids/figlens/commit/dc257f64a95e1a916a131d2cec8413f63a1947ed))
* **preview:** return keyboard focus to the Relations result that opened a layer ([#145](https://github.com/runkids/figlens/issues/145)) ([0c44c95](https://github.com/runkids/figlens/commit/0c44c9561aa4f45bf5cd0300448c639d2b077777))
* **render:** draw STRETCH image fills once instead of repeating past the edges ([#110](https://github.com/runkids/figlens/issues/110)) ([6488aa8](https://github.com/runkids/figlens/commit/6488aa8d285f757a11ac0cd83b99ab9f94d529a2))
* **render:** embed each image once per SVG instead of once per fill ([#135](https://github.com/runkids/figlens/issues/135)) ([c715aad](https://github.com/runkids/figlens/commit/c715aaddbd451881a605c4e45537de6182da731f))


### Performance

* **export:** optimize PNG output losslessly ([#154](https://github.com/runkids/figlens/issues/154)) ([a06e851](https://github.com/runkids/figlens/commit/a06e851e4222659134e26a599a138d2eb4b75b67))
* **search:** hex-encode digests without a format! per byte ([#144](https://github.com/runkids/figlens/issues/144)) ([5b8d5cb](https://github.com/runkids/figlens/commit/5b8d5cbdf6b6703d474f5382fb9a334a9bcef46e))
* **watch:** compare tracked builds in the index worker ([#158](https://github.com/runkids/figlens/issues/158)) ([b64fc90](https://github.com/runkids/figlens/commit/b64fc901c2d7cca2a511053ecffe280699c58e13))
* **watch:** cut stable-save-to-event latency on the detector, facet and owner paths ([#149](https://github.com/runkids/figlens/issues/149)) ([ee1fa8f](https://github.com/runkids/figlens/commit/ee1fa8f7b2a4d1cf306f3938242e18938b145a5b))

## [0.9.3](https://github.com/runkids/figlens/compare/v0.9.2...v0.9.3) (2026-10-29)


### New Features

* **export:** export at the layout box and report both boxes ([#86](https://github.com/runkids/figlens/issues/86)) ([2e71d4e](https://github.com/runkids/figlens/commit/2e71d4e5844a4a5e6378b0492081d5d605fc1579))
* **export:** export many nodes from one decode ([#84](https://github.com/runkids/figlens/issues/84)) ([a2b9211](https://github.com/runkids/figlens/commit/a2b9211514e5e0cace07172e1c20d4b34e5b5654))
* **render:** decode and draw per-side stroke weights ([#96](https://github.com/runkids/figlens/issues/96)) ([d832069](https://github.com/runkids/figlens/commit/d83206940db8ca6b3a68891169dc259328c3a65f))
* **tree:** include size and visibility in --json ([#99](https://github.com/runkids/figlens/issues/99)) ([d1f63b2](https://github.com/runkids/figlens/commit/d1f63b2dca4191d2ada3348dedb98dbb38f27ac3))


### Bug Fixes

* **tree:** print the REST node types nodes and find use ([#97](https://github.com/runkids/figlens/issues/97)) ([0363bb8](https://github.com/runkids/figlens/commit/0363bb8fe2f4c5e27cc218fbac54d9a96bc3a975))

## [0.9.2](https://github.com/runkids/figlens/compare/v0.9.1...v0.9.2) (2026-10-29)


* release 0.9.2 ([088ba1f](https://github.com/runkids/figlens/commit/088ba1f4c5ffd312e108dc65c578aab6a066eed4))


### New Features

* **verify:** add scoped handoff preflight findings ([#67](https://github.com/runkids/figlens/issues/67)) ([4e6eba9](https://github.com/runkids/figlens/commit/4e6eba931710f732085f76bf0fdc71c62589b145))


### Bug Fixes

* **export:** refuse hidden layers with a clear error in every format ([#73](https://github.com/runkids/figlens/issues/73)) ([51653f2](https://github.com/runkids/figlens/commit/51653f251f18f8a443b19096d9fa5d875f825ad5))
* **mcp:** bound instance expansion for nodes on a stored root ([#71](https://github.com/runkids/figlens/issues/71)) ([0f177c1](https://github.com/runkids/figlens/commit/0f177c118efe96d28d341fd33141f1a9e729f195))
* **mcp:** stop building oversized nodes JSON before the reply check ([#70](https://github.com/runkids/figlens/issues/70)) ([5fa7e6e](https://github.com/runkids/figlens/commit/5fa7e6ef07536daf405d2d40c59a1e47bdd5b0d6))
* **render:** tile TILE image fills instead of drawing them as FILL ([#72](https://github.com/runkids/figlens/issues/72)) ([09e7b06](https://github.com/runkids/figlens/commit/09e7b06de682b4ad96d0541f73e827cea3b8dba1))

## [0.9.1](https://github.com/runkids/figlens/compare/v0.9.0...v0.9.1) (2026-10-28)


### Bug Fixes

* **opencode:** run the bundled launcher with node, not the host binary ([#64](https://github.com/runkids/figlens/issues/64)) ([5e6a36a](https://github.com/runkids/figlens/commit/5e6a36aceeb62577e62c0283773cdbfc03215b5b))
* **pi:** describe every /figlens subcommand, speed up CI, document opencode plugin add ([#63](https://github.com/runkids/figlens/issues/63)) ([18d4552](https://github.com/runkids/figlens/commit/18d45526d1dc6ddc859332acd64049e6c5f3af6b))

## [0.9.0](https://github.com/runkids/figlens/compare/v0.8.0...v0.9.0) (2026-10-28)


### New Features

* add evidence-based UI audit workflow ([d9516ec](https://github.com/runkids/figlens/commit/d9516ecb511062db7cc126ac142b394c44f181ca))
* add evidence-based UI audit workflow ([0442286](https://github.com/runkids/figlens/commit/04422868fef73392f82061bee3eacc04e9b0df82))
* **bundle:** add font inventory and verify readiness check ([d6e273c](https://github.com/runkids/figlens/commit/d6e273c252bc062e7ed9f0cb703bc6fad37a6579))
* **bundle:** add font inventory and verify readiness check ([3f2c85b](https://github.com/runkids/figlens/commit/3f2c85ba66bbad12a21f9cec4102af1609803b19))
* **cli:** print built-in agent guidance with `figlens workflow` ([d911f0e](https://github.com/runkids/figlens/commit/d911f0e701be64e63673e507b91509b825cf61f1))
* **cli:** print built-in agent guidance with `figlens workflow` ([e20bc05](https://github.com/runkids/figlens/commit/e20bc05652994f6740a014e97bef7f6276e92dc3))
* **cli:** read many nodes from one decode with nodes --frames ([79fd078](https://github.com/runkids/figlens/commit/79fd078c4a148409ddda788f45903c016ec749db))
* **cli:** read many nodes from one decode with nodes --frames ([1b11a2a](https://github.com/runkids/figlens/commit/1b11a2aaeb1e5d9c95e0e56f1156163e57182f13)), closes [#52](https://github.com/runkids/figlens/issues/52)
* **cli:** take nodes batch ids from stdin or arguments ([599bf88](https://github.com/runkids/figlens/commit/599bf88bb94725b4be7debea02caa47f0595f2ff))
* **integrations:** add OpenCode and OMP plugin entries ([#56](https://github.com/runkids/figlens/issues/56)) ([964aa13](https://github.com/runkids/figlens/commit/964aa130a7e29c73622d4e5b54c23e600010a983))


### Bug Fixes

* **bundle:** look font fields up instead of cloning styles ([6c93701](https://github.com/runkids/figlens/commit/6c93701e680ad97bccf38349a6382c6dec148274))
* **bundle:** tolerate text styles that are not objects ([9a0a703](https://github.com/runkids/figlens/commit/9a0a70399852b0a9d311256d46cf3716751cc6ae))
* **guidance:** carry the ignore-file step to every output instruction ([5d70db5](https://github.com/runkids/figlens/commit/5d70db5adff48d7940f54d2b1a5f6e31b2fd799d))
* **guidance:** keep agent reference outputs out of the project tree ([67ed1ca](https://github.com/runkids/figlens/commit/67ed1ca52169f88fe49decfedb89258dafe6319e))
* **guidance:** keep agent reference outputs out of the project tree ([a96ba49](https://github.com/runkids/figlens/commit/a96ba494dc72c8bb20a4ab6b3aaf4ca3bca57262))
* **guidance:** send detailed-plan exports to .figlens/exports ([388f260](https://github.com/runkids/figlens/commit/388f2604a2303042f0ecd19894d10f6233db00b3))
* **guidance:** stop after a whole-scope plan instead of starting slice 1 ([d87af7e](https://github.com/runkids/figlens/commit/d87af7e63741c80ac1e781b258efeb7afaf8d5e4))
* **guidance:** stop after a whole-scope plan instead of starting slice 1 ([56bcca7](https://github.com/runkids/figlens/commit/56bcca7243ad128713d0460109cdaa145592ccc4))
* **render:** bound the cuts that let background blurs replace their backdrop ([549cc9d](https://github.com/runkids/figlens/commit/549cc9dd99faee1e26b859da7af85ddde8b1349e))
* **render:** cut a node's own background blur under a nested one ([ab488dc](https://github.com/runkids/figlens/commit/ab488dcdb0868c32d5e3adf4dcbac1c2e0acd65f))
* **render:** draw background blur and stop blurring unknown effects ([5c551a3](https://github.com/runkids/figlens/commit/5c551a3fb6a65b3b21625a74f0d824275bdbf4e1))
* **render:** draw background blur and stop blurring unknown effects ([712d939](https://github.com/runkids/figlens/commit/712d939111b2a1351c6996dcdb6ced032c135331))
* **render:** draw no background blur inside mask content ([df4daf8](https://github.com/runkids/figlens/commit/df4daf8b2e2afbc5009c437bbaa2969838a0b7ba))
* **render:** keep background blur backdrops linear and bounded ([63d2558](https://github.com/runkids/figlens/commit/63d255878e7e28d34be807d755c3d08f197df343))
* **render:** let background blur replace its backdrop ([4e26a28](https://github.com/runkids/figlens/commit/4e26a28431816e0233458d3aa22d37f3558244ea))
* **render:** tile the mirrored backdrop as far as a background blur reads ([e3e2c38](https://github.com/runkids/figlens/commit/e3e2c3887a09833412f08c428a131fe0442b0835))
* **verify:** bound total hashing and keep reports for damaged metadata ([4e7bbf4](https://github.com/runkids/figlens/commit/4e7bbf4fc1e502366617cab0a46802a4ba474614))
* **verify:** report undecodable entries instead of aborting ([5e856cc](https://github.com/runkids/figlens/commit/5e856cc1932b3a7cb0e6fd59f703c46f7c129b60))
* **verify:** reserve declared sizes from the hashing budget ([6f36bdd](https://github.com/runkids/figlens/commit/6f36bddcd5237ddc78b5ce44a6cc63e7a1d679b5))

## [0.8.0](https://github.com/runkids/figlens/compare/v0.7.4...v0.8.0) (2026-10-28)


### ⚠ BREAKING CHANGES

* **mcp:** MCP tool calls with fields outside the tool's input schema now fail with `invalid_arguments` instead of being ignored. Use the parameter names from `tools/list`.

### New Features

* **mcp:** reject unknown tool arguments ([2f60ea5](https://github.com/runkids/figlens/commit/2f60ea5737241a8ae73b6985a41e619e8e2aab37))


### Bug Fixes

* **mcp:** keep advertised schema integers JSON-safe ([ceee44a](https://github.com/runkids/figlens/commit/ceee44ace3979dda4418dba009d661e88008c364))

## [0.7.4](https://github.com/runkids/figlens/compare/v0.7.3...v0.7.4) (2026-10-27)


### New Features

* **handoff:** document offline UI case and dependency preflight ([#40](https://github.com/runkids/figlens/issues/40)) ([6e5fac2](https://github.com/runkids/figlens/commit/6e5fac282707f69f59f5c46f52d4ffeb723cafb4))


### Bug Fixes

* **render:** omit SVG group identifiers and provenance ([afe3067](https://github.com/runkids/figlens/commit/afe30670136029c3733b46655c7143ff68ded2ce))
* **render:** scope SVG ids to the exported node ([321e8e6](https://github.com/runkids/figlens/commit/321e8e6188e5d3bf2955737ef70175441086e4a2))
* **render:** scope SVG ids to the exported node ([10609e4](https://github.com/runkids/figlens/commit/10609e43ec6865a6edf28fd4a01c214c7fa0a1ab))

## [0.7.3](https://github.com/runkids/figlens/compare/v0.7.2...v0.7.3) (2026-10-27)


### Bug Fixes

* handle subcommand help and bound original image reads ([ee79f78](https://github.com/runkids/figlens/commit/ee79f78c962b7224c3e40eaf3a9050df20c6fd86))
* protect outputs and bound MCP handoffs ([e12a3d0](https://github.com/runkids/figlens/commit/e12a3d0ed37dc537bb71df8de137d02bd8549075))
* protect outputs and bound MCP handoffs ([dd315dd](https://github.com/runkids/figlens/commit/dd315dd2f0cae966f905c14bb964808902cbfc7d))

## [0.7.2](https://github.com/runkids/figlens/compare/v0.7.1...v0.7.2) (2026-10-26)


### New Features

* compare stored component variants and apply text properties ([61614f8](https://github.com/runkids/figlens/commit/61614f8b8c9b79e4a0f3103430f8400fc7bd932d))
* expose component state evidence and variant comparisons ([f875c0d](https://github.com/runkids/figlens/commit/f875c0df29089e1011c30c6e63e660c95a4608e5))
* expose component variants and prototype evidence ([7ed5310](https://github.com/runkids/figlens/commit/7ed5310f1f5fbfa557a780ab1d7516a8e0c77671))
* expose partial layout semantics and render diagnostics ([eed74be](https://github.com/runkids/figlens/commit/eed74bed7b23b76b41098a5f74c0a96a621a1200))
* expose partial layout semantics and render diagnostics ([6702292](https://github.com/runkids/figlens/commit/67022926b8a8c6acefc8a6f3e17cb887721442ef))


### Bug Fixes

* compare stored left and top padding fields ([9fe6dc1](https://github.com/runkids/figlens/commit/9fe6dc118098721fbe113980a25952035aa504b6))
* preserve text metadata and empty prototype status rows ([b1bf5b0](https://github.com/runkids/figlens/commit/b1bf5b0e6778cb160e902b1811f2ae3db72e29b8))
* reject malformed component parent memberships ([a072da8](https://github.com/runkids/figlens/commit/a072da8b07d764b7c0d77be078acdd86810e1dfd))
* reject malformed GUIDs in shared reference lookups ([e6ea189](https://github.com/runkids/figlens/commit/e6ea1898ba79bdd41a2ecbc29dfe0c357e78e5a4))


### Performance

* **render:** defer stroke geometry checks for diagnostics ([b87cafe](https://github.com/runkids/figlens/commit/b87cafec9f25fe1cf60c39852b433524aa7b1e5f))
* **render:** skip unused pure-blur intermediates ([eb01fe3](https://github.com/runkids/figlens/commit/eb01fe35deb84a829b27a1197e2b123dd05d3121))
* **render:** skip unused pure-blur intermediates ([798fe20](https://github.com/runkids/figlens/commit/798fe20343a790a44b53ceb8090b0e4e924af808))
* skip unused parent definition preparation ([e96b907](https://github.com/runkids/figlens/commit/e96b9073b995450740add80061e89fb17b9d73c1))

## [0.7.1](https://github.com/runkids/figlens/compare/v0.7.0...v0.7.1) (2026-10-25)


### Performance

* avoid full-file scans when opening fig archives ([26cc0a9](https://github.com/runkids/figlens/commit/26cc0a949246b84951e5d63fec28195dc5d775a8))
* avoid full-file scans when opening fig archives ([e93212d](https://github.com/runkids/figlens/commit/e93212db3bcb7cb276eef341f7615de37d023414))
* bound frames only with --jobs and release message bytes after decode ([0e10d14](https://github.com/runkids/figlens/commit/0e10d142726fad08ec82ca395f1116b19542915f))
* decouple frame jobs and release decoded message bytes ([eec2968](https://github.com/runkids/figlens/commit/eec29685aee79b4a218634d67699ee26d298fbb8))
* store decoded Kiwi byte arrays compactly ([4ec0f36](https://github.com/runkids/figlens/commit/4ec0f368825a19a6d2191744b4897cf11df0b3df))
* store decoded Kiwi byte arrays compactly ([2ab7ad5](https://github.com/runkids/figlens/commit/2ab7ad5250a89a23c061253e00248c8bac2ec4fa))

## [0.7.0](https://github.com/runkids/figlens/compare/v0.6.0...v0.7.0) (2026-10-25)


### ⚠ BREAKING CHANGES

* **mcp:** MCP no longer accepts legacy CLI argument coercions, numeric strings or optional null/false sentinels. Use JSON numbers and maxSize: 0 for uncapped exports.

* **mcp:** centralize tool contracts and request admission ([1309faa](https://github.com/runkids/figlens/commit/1309faa1b13eefcc239ebfbd1e1fecaa25e6c520))


### New Features

* expose built-in MCP workflow guidance ([8de61d8](https://github.com/runkids/figlens/commit/8de61d89e60b53e83a081d38c03675124e6fed3e))


### Bug Fixes

* align MCP contracts and clean up smoke processes ([7dcbe8b](https://github.com/runkids/figlens/commit/7dcbe8b48507a9526ffa40716cf5cc05f974369e))
* bound document parsing before Kiwi decoding ([b6e959f](https://github.com/runkids/figlens/commit/b6e959f44148f62f74fdc4175b46a5dc27089a58))
* create candidate output before container builds ([bb45fed](https://github.com/runkids/figlens/commit/bb45fed6af1e5d446b04ccab13520565fbfc28e5))
* harden design parsing and verify installed workflows ([3c482e8](https://github.com/runkids/figlens/commit/3c482e85918d5cdf661bb0e6b2fd32a0769bbb1d))
* resolve instance sublayer IDs consistently ([b9654ae](https://github.com/runkids/figlens/commit/b9654aea4ec618324337d05f64dc70dd748ac2af))
* use Windows system tar for ZIP extraction ([dd33bd2](https://github.com/runkids/figlens/commit/dd33bd258d8eb90f58ae9456b6138b2d205ba137))


### Reverts

* **skill:** drop sizing-intent guidance from the MCP refactor ([b1c1f9d](https://github.com/runkids/figlens/commit/b1c1f9d3092360a22b32d6db5ef408dcc58bfa4c))

## [0.6.0](https://github.com/runkids/figlens/compare/v0.5.0...v0.6.0) (2026-10-24)


### New Features

* add autonomous design implementation workflow ([19d6d08](https://github.com/runkids/figlens/commit/19d6d08d396d14c9a1daa93328f66ca8fb5bf2f0))

## [0.5.0](https://github.com/runkids/figlens/compare/v0.4.2...v0.5.0) (2026-10-24)


### New Features

* add scoped design planning, image evidence and Pi preview ([#16](https://github.com/runkids/figlens/issues/16)) ([36ecf0b](https://github.com/runkids/figlens/commit/36ecf0b44e3b703550597e6cd6f67076fa8964df))

## [0.4.2](https://github.com/runkids/figlens/compare/v0.4.1...v0.4.2) (2026-10-23)


### Bug Fixes

* **agent:** prepare website images using project conventions ([#14](https://github.com/runkids/figlens/issues/14)) ([d5569e8](https://github.com/runkids/figlens/commit/d5569e8786376f5fc57af8fbd074977cb2aad486))

## [0.4.1](https://github.com/runkids/figlens/compare/v0.4.0...v0.4.1) (2026-10-23)


* **skill:** build with the project's components and tokens before raw values ([#12](https://github.com/runkids/figlens/issues/12)) ([961bfbb](https://github.com/runkids/figlens/commit/961bfbba385e3008615d656308f2498a8d417757))

## [0.4.0](https://github.com/runkids/figlens/compare/v0.3.3...v0.4.0) (2026-10-23)


### New Features

* export maxSize, --root folders and review fixes ([edf8a83](https://github.com/runkids/figlens/commit/edf8a833a6fd3137b701cf13e9e4d6d2e0d14de7))
* **export:** cap the longest side with maxSize, 1600 px by default over MCP ([9cdf2be](https://github.com/runkids/figlens/commit/9cdf2be4fb8a9fb759f9854e6917e768c464214c))
* **mcp:** confine files and outputs to --root folders ([3b52502](https://github.com/runkids/figlens/commit/3b52502a75e0c7611f699257d71272fbc919a381))
* **mcp:** keep the parsed file across calls on the same file ([10faa13](https://github.com/runkids/figlens/commit/10faa130aef8a928c1ad73e7c790195f318c2130))
* **mcp:** keep the parsed file across calls on the same file ([93cbf70](https://github.com/runkids/figlens/commit/93cbf7024526d733b884528a2ac7d65c5aecc8a8))


### Bug Fixes

* **mcp:** close three --root and tree-reuse gaps found in review ([a4e6077](https://github.com/runkids/figlens/commit/a4e6077ea72b8ce69002ebbeee20e84cf42f6aa1))
* **mcp:** keep review findings out of the parse reuse and export cap ([948595c](https://github.com/runkids/figlens/commit/948595c9a0b7f11b1d74859e02b1839fce221c92))
* **mcp:** never preload the file of a tools/call notification ([d76270d](https://github.com/runkids/figlens/commit/d76270d60c0d3d5c9e55d450ed23e3bb455c7b5c))

## [0.3.3](https://github.com/runkids/figlens/compare/v0.3.2...v0.3.3) (2026-10-23)


### Bug Fixes

* **render:** preserve offset image fills in clipped exports ([c90a355](https://github.com/runkids/figlens/commit/c90a355c17d943b900d25aceeb810b1a475ef12c))

## [0.3.2](https://github.com/runkids/figlens/compare/v0.3.1...v0.3.2) (2026-10-23)


### New Features

* **export:** support direct SVG output ([0f91f42](https://github.com/runkids/figlens/commit/0f91f421b381cfc39a3168c85169517f3fd4a175))

## [0.3.1](https://github.com/runkids/figlens/compare/v0.3.0...v0.3.1) (2026-10-23)


### Bug Fixes

* **pi:** make local design workflows agent-first ([0b22ffe](https://github.com/runkids/figlens/commit/0b22ffed66231d4331dde8ab93770cc66d4efd10))

## [0.3.0](https://github.com/runkids/figlens/compare/v0.2.0...v0.3.0) (2026-10-22)


### New Features

* add agent discovery and inline MCP images ([609ec85](https://github.com/runkids/figlens/commit/609ec85387e8735c34f4ee9e20daf437f8ef9e20))
* **pi:** reuse reviewed project components from local designs ([8ffcc85](https://github.com/runkids/figlens/commit/8ffcc858359fa148d726985c797ae1667de2bee0))
* **preview:** add a Figma-style local design viewer ([b36384a](https://github.com/runkids/figlens/commit/b36384a90d85a1a5596bebe36f0ba19fc9799e31))
* **preview:** browse a component's variants from the inspector ([d3c8f29](https://github.com/runkids/figlens/commit/d3c8f294714069a034f1e2f7a45b5cca6b9c8163))
* **preview:** export the selection as SVG ([eb49d8d](https://github.com/runkids/figlens/commit/eb49d8d2ae768d22fca2ce12e21c21a93457f413))
* **preview:** pick rotated layers, ellipses and lines by their shape ([6ade124](https://github.com/runkids/figlens/commit/6ade124529c9c398ae1a57d938d697c2a888206d))
* **preview:** pick vector layers by their outlines ([195978b](https://github.com/runkids/figlens/commit/195978be951087b9ef2238dc78bb9010b7352dd9))
* **preview:** select several layers with Shift-click ([b7c2eca](https://github.com/runkids/figlens/commit/b7c2ecae9e6acf68e517fb24ca7bacae8f22aa0d))
* **preview:** show prototype interactions in the inspector ([a5111a9](https://github.com/runkids/figlens/commit/a5111a9feb0891254f22249b351d11079b2fe1d4))


### Bug Fixes

* **npm:** preserve the binary during concurrent first use ([5e87824](https://github.com/runkids/figlens/commit/5e8782411e7b76398a4e6c27d68a0678a149bbad))

## [0.2.0](https://github.com/runkids/figlens/compare/v0.1.2...v0.2.0) (2026-10-21)


### ⚠ BREAKING CHANGES

* export prints a JSON object instead of "<path>: WxH, N KB, quality Q"; read width, height, bytes and quality from it. bundle prints a JSON object on stdout and no longer repeats its warnings on stderr.

### New Features

* print export and bundle results as JSON ([42b15c3](https://github.com/runkids/figlens/commit/42b15c36cab89be736c656409456fb9915afb16c))

## [0.1.2](https://github.com/runkids/figlens/compare/v0.1.1...v0.1.2) (2026-10-21)


* publish to npm through OIDC trusted publishing ([45b705b](https://github.com/runkids/figlens/commit/45b705bcffe4409011663cbe2451fa90d01b2125))

## [0.1.1](https://github.com/runkids/figlens/compare/v0.1.0...v0.1.1) (2026-10-21)


### Bug Fixes

* publish the npm wrapper as @runkids/figlens ([f46333b](https://github.com/runkids/figlens/commit/f46333b323d7748a95e9333c650c6c0da0725425))

## 0.1.0 (2026-10-21)


### New Features

* read Figma .fig files offline ([e77dbd6](https://github.com/runkids/figlens/commit/e77dbd63e3490405a0f7a4a7895a13c689d90ced))
* serve figlens tools over MCP ([c411c2c](https://github.com/runkids/figlens/commit/c411c2c70fda54017e31c8941bf262f3d069c128))


### Bug Fixes

* harden reading crafted .fig files ([2ccc446](https://github.com/runkids/figlens/commit/2ccc446830c0a7aec0d9461468f6850b411d3023))
