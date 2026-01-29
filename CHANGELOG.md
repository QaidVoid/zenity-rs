
## [0.1.10](https://github.com/QaidVoid/zenity-rs/compare/v0.1.9...v0.1.10) - 2026-01-29

### Added

- Add scrollbar thumb dragging - ([61a6895](https://github.com/QaidVoid/zenity-rs/commit/61a6895a3181276ae6489b7ec119119637759fe3))
- Add stdin support for list dialog - ([84ff3f7](https://github.com/QaidVoid/zenity-rs/commit/84ff3f7d2efad336eacef790c106bf7b9c3b06ad))

### Other

- Sync hive - ([b9a4fa5](https://github.com/QaidVoid/zenity-rs/commit/b9a4fa5067f28d1ff7ffdb630c084a74add30584))
- Integrate RingBufferCache in text_info.rs - ([10c96db](https://github.com/QaidVoid/zenity-rs/commit/10c96dbfe34c28e1b46b0d4cd7bb3ee51dacabce))
- Sync hive - ([16d3338](https://github.com/QaidVoid/zenity-rs/commit/16d3338b5e04a2b30e9b95fa8a5676f6108e484c))
- Sync hive - ([be3d97e](https://github.com/QaidVoid/zenity-rs/commit/be3d97e43afea39bb816b598a4c8ac9b80786d4a))
- Update build command - ([a43d42a](https://github.com/QaidVoid/zenity-rs/commit/a43d42a4b003d8c00287e1abb83ca05f837efb15))

## [0.1.9](https://github.com/QaidVoid/zenity-rs/compare/v0.1.8...v0.1.9) - 2026-01-29

### Added

- Show mounted drives in file selector - ([c529bc5](https://github.com/QaidVoid/zenity-rs/commit/c529bc5841fabc6b7c475706b52e3e3ae3d1b964))

### Fixed

- Prevent breadcrumb overflow in file selector - ([ed819e6](https://github.com/QaidVoid/zenity-rs/commit/ed819e6f81c3dae55c2b425ec5bbbd7441f49e2c))
- Wrap text in entry and forms dialogs - ([72fb1ca](https://github.com/QaidVoid/zenity-rs/commit/72fb1ca74842b5274b973823be51cfef992b2cb2))

### Other

- Use /run/mount/utab instead of /proc/mounts - ([f1916d5](https://github.com/QaidVoid/zenity-rs/commit/f1916d5a8cad5eeb2cb8cae80c79740270fe47b5))

## [0.1.8](https://github.com/QaidVoid/zenity-rs/compare/v0.1.7...v0.1.8) - 2026-01-23

### Fixed

- Backend selection bug when WAYLAND_DISPLAY is set - ([c397c8b](https://github.com/QaidVoid/zenity-rs/commit/c397c8b2426691e680302d244c2ee36890c7f780))

## [0.1.7](https://github.com/QaidVoid/zenity-rs/compare/v0.1.6...v0.1.7) - 2026-01-23

### Added

- Support multiple patterns and named file filters - ([7ad187b](https://github.com/QaidVoid/zenity-rs/commit/7ad187b8931f2adfbf4f05d64d735fea781c6568))
- Add --no-wrap option - ([a8b13e3](https://github.com/QaidVoid/zenity-rs/commit/a8b13e3b1ee5498a904440f22b96d8c1810bcbe7))

## [0.1.6](https://github.com/QaidVoid/zenity-rs/compare/v0.1.5...v0.1.6) - 2026-01-23

### Added

- Add --multiple to list and fix modifier state tracking - ([d825911](https://github.com/QaidVoid/zenity-rs/commit/d825911ed208072ae61286602876fcd374ad7721))
- Add --separator support for list - ([04f6f10](https://github.com/QaidVoid/zenity-rs/commit/04f6f109b710bea552a9714823b51af5312fa509))

### Fixed

- Share same separator with different options - ([299e2f8](https://github.com/QaidVoid/zenity-rs/commit/299e2f876b03f3673675da7345cbb917a6e099d6))
- Traverse symlink for metadata - ([eaff044](https://github.com/QaidVoid/zenity-rs/commit/eaff044f0ffe040700590c54eab373ad00e70ed6))

## [0.1.5](https://github.com/QaidVoid/zenity-rs/compare/v0.1.4...v0.1.5) - 2026-01-23

### Added

- Add --multiple and --separator options for file selection dialog - ([fd58857](https://github.com/QaidVoid/zenity-rs/commit/fd588570f062709516184bfdeae43e78ba672009))
- Add --file-filter option for file selection dialog - ([ec94246](https://github.com/QaidVoid/zenity-rs/commit/ec9424611f5c28eb68776f6afa7aa106d46ec7c0))
- Add dialog borders and subtle shadow - ([7b6485f](https://github.com/QaidVoid/zenity-rs/commit/7b6485fc7b6c2cad8fea96e0546e6b7bc9a7c517))

### Fixed

- Improve wayland detection ([#11](https://github.com/QaidVoid/zenity-rs/pull/11)) - ([3ad3c2d](https://github.com/QaidVoid/zenity-rs/commit/3ad3c2df06cdb49d806bb83c7528fb3b917bfbd3))
- Make entry dialog respect custom width/height parameters - ([072c358](https://github.com/QaidVoid/zenity-rs/commit/072c358b2d5f15ca517bcd12d7f8b1173688c02d))
- Make default calendar title same as gtk zenity ([#9](https://github.com/QaidVoid/zenity-rs/pull/9)) - ([dd0f89c](https://github.com/QaidVoid/zenity-rs/commit/dd0f89c37e23eb34bbe161cfec7596d0fb06f68d))

## [0.1.4](https://github.com/QaidVoid/zenity-rs/compare/v0.1.3...v0.1.4) - 2026-01-21

### Fixed

- Set wayland appid as zenity - ([b735560](https://github.com/QaidVoid/zenity-rs/commit/b735560cad6baf30c2b6fd7738ec2a2bed80feac))
- Use display_rows for col_widths calculation when columns are hidden - ([176fa91](https://github.com/QaidVoid/zenity-rs/commit/176fa91a9c415d85f9c6f70c4b0b1f1b54561ca9))

### Other

- Make window class same as original zenity ([#7](https://github.com/QaidVoid/zenity-rs/pull/7)) - ([b85f389](https://github.com/QaidVoid/zenity-rs/commit/b85f3897073021a11bf7dc4ee81f50d411f1d40f))
- Remove truncation logic in list - ([076dd69](https://github.com/QaidVoid/zenity-rs/commit/076dd696a9949e5016bf420d9c41f83f9a4a04ce))

## [0.1.3](https://github.com/QaidVoid/zenity-rs/compare/v0.1.2...v0.1.3) - 2026-01-21

### Fixed

- Fix cursor theme when switching from I beam ([#6](https://github.com/QaidVoid/zenity-rs/pull/6)) - ([ffbc51e](https://github.com/QaidVoid/zenity-rs/commit/ffbc51e149fbcdd079c6008bc60cd9428055e7c2))

### Other

- Add --no-cancel and --time-remaining on progress - ([ddddb11](https://github.com/QaidVoid/zenity-rs/commit/ddddb11aca5ccdc5dfec981479589b2a3a49f17a))
- Add --auto-kill option to progress dialog - ([1e29c63](https://github.com/QaidVoid/zenity-rs/commit/1e29c63d5e317caf8a97a5bbd78909eaa5560291))
- Add horizontal scrolling and text clipping to list dialog - ([d09eab0](https://github.com/QaidVoid/zenity-rs/commit/d09eab04cab4fa741cf930c604e9e0027b71e416))
- Format code - ([f99f51b](https://github.com/QaidVoid/zenity-rs/commit/f99f51bce3c6b36672d1ed2992cabe2e1e3f4b3e))
- Ignore --modal flag for zenity compatibility - ([be82f30](https://github.com/QaidVoid/zenity-rs/commit/be82f3068ec461a24ed3eedc2e8f9a64d4b6ad7b))

## [0.1.2](https://github.com/QaidVoid/zenity-rs/compare/v0.1.1...v0.1.2) - 2026-01-19

### Other

- Fix --hide-column index for radiolist/checklist mode - ([4cf13e9](https://github.com/QaidVoid/zenity-rs/commit/4cf13e9fd5967fc10ff50307dcebe3b4f7ab1c8e))
- Add --hide-column option for list dialogs - ([b6cfa2f](https://github.com/QaidVoid/zenity-rs/commit/b6cfa2f8d23199f7be484d1adcd0df9e3c1934d3))
- Improve message dialog text handling - ([90d7296](https://github.com/QaidVoid/zenity-rs/commit/90d72965c79b56ae2422df05dd97ca1b2249b630))

## [0.1.1](https://github.com/QaidVoid/zenity-rs/compare/v0.1.0...v0.1.1) - 2026-01-19

### Other

- Reorganize help to show options per dialog type - ([1b40ea9](https://github.com/QaidVoid/zenity-rs/commit/1b40ea9f43d71a448b68a2db1ecd9c2a3aa838e3))
- Add text cursor to entry dialog input field - ([8208ebc](https://github.com/QaidVoid/zenity-rs/commit/8208ebc796c02e789472cce3838c34e79c10ca9d))
- Add cursor shape support for text input fields - ([0a92d73](https://github.com/QaidVoid/zenity-rs/commit/0a92d7368c206b6f177275564d61875d2c3bebb1))
- Add forms dialog to README - ([9044129](https://github.com/QaidVoid/zenity-rs/commit/9044129da0ba00ea48a785bb4d1ad85c5aa08c5c))
- Add forms dialog for multiple input fields - ([dd28d45](https://github.com/QaidVoid/zenity-rs/commit/dd28d451e782dba186346657caa50a51b80bb42e))
- Update README with text-info and scale dialogs - ([4649955](https://github.com/QaidVoid/zenity-rs/commit/464995576f297a212a159abae52b4729df728f63))
- Add --scale dialog for selecting numeric values with a slider - ([e33341d](https://github.com/QaidVoid/zenity-rs/commit/e33341d4c7fb955b44c5f0152fe0ef5858e9504d))
- Add --text-info dialog for displaying scrollable text - ([c6b1705](https://github.com/QaidVoid/zenity-rs/commit/c6b17057602469e0c4dab668532fe591e103d1a8))
- Add --width and --height CLI flags for custom dialog dimensions - ([89f9497](https://github.com/QaidVoid/zenity-rs/commit/89f94979653d6702ab20364f41d70d867bd036cf))
- Fix Wayland poll_for_event not reading events from socket - ([5f9f4d8](https://github.com/QaidVoid/zenity-rs/commit/5f9f4d8c1601ddb3f763d62a5faf34d858384bcf))

## [0.1.0] - 2026-01-18

### Other

- Add release workflows for automated releases - ([c4d8c55](https://github.com/QaidVoid/zenity-rs/commit/c4d8c5568af0196c11f56af5ec6e50f87dfa69c7))
- Add HiDPI scaling support for crisp text rendering - ([f0663c4](https://github.com/QaidVoid/zenity-rs/commit/f0663c45e59e8ebe82330caede5c6085ffee84c8))
- Improve calendar dialog with month/year dropdowns - ([948e5a1](https://github.com/QaidVoid/zenity-rs/commit/948e5a18932f3ca931ea1a403adf50f34d72b401))
- Set rust-toolchain to nightly - ([060640a](https://github.com/QaidVoid/zenity-rs/commit/060640a32a249a86863876be97d4c8b55932dcf3))
- Add README - ([8f068ed](https://github.com/QaidVoid/zenity-rs/commit/8f068ed18ce7c96adec4a15b02a7d852c840c740))
- Fix keyboard layout handling - ([60bc2d6](https://github.com/QaidVoid/zenity-rs/commit/60bc2d6377ec21233cd7e4d31cb0911279883067))
- Fix unused assignment warnings in list dialog - ([96dc38b](https://github.com/QaidVoid/zenity-rs/commit/96dc38b5e43a707e000c044fd3e4de516ecc8728))
- Show help when run without arguments - ([16107c1](https://github.com/QaidVoid/zenity-rs/commit/16107c189664eb6ea23c1339ee5c221417a111a4))
- Rename to zenity-rs, add timeout support, cleanup - ([f64043d](https://github.com/QaidVoid/zenity-rs/commit/f64043d1bbafb593e180b3583ff6b35851ae9133))
- Add list selection and calendar dialogs - ([fd9a84f](https://github.com/QaidVoid/zenity-rs/commit/fd9a84fb0928a0e2a6716fafb678b56031b528b6))
- Enhance file selection dialog with modern UI - ([65a6d42](https://github.com/QaidVoid/zenity-rs/commit/65a6d42b9d90763378a53ed095550a9fd7dafc5d))
- Add progress and file selection dialogs - ([1d0f106](https://github.com/QaidVoid/zenity-rs/commit/1d0f1069c263b2754f0d4b1d863c6a0e79f86d4a))
- Add entry and password dialogs - ([3feea3a](https://github.com/QaidVoid/zenity-rs/commit/3feea3a41cebe7d3e4f8ed6dba08b6b2bfc79b75))
- Initial implementation of rask - ([784e67f](https://github.com/QaidVoid/zenity-rs/commit/784e67fc371e5f84f28157b180b965335725d329))
- Initial commit - ([ff0b14a](https://github.com/QaidVoid/zenity-rs/commit/ff0b14a02069fa7a96950ae87d9c8f8443d988f7))
