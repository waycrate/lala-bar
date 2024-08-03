project(
  'lala_musicbar',
  'rust',
  version: '0.2.5',
  license: 'MIT',
  meson_version: '>= 1.0',
)

cargo = find_program('cargo', required: true, version: '>=1.80')
find_program('rustc', required: true, version: '>=1.80')
if get_option('debug')
  command = [
    cargo,
    'build',
    '&&',
    'cp',
    meson.global_source_root() / 'target' / 'debug' / meson.project_name(),
    '@OUTPUT@',
  ]
else
  command = [
    cargo,
    'build',
    '--release', '&&',
    'cp',
    meson.global_source_root() / 'target' / 'release' / meson.project_name(),
    '@OUTPUT@',
  ]
endif

prefix = get_option('prefix')

bindir = prefix / get_option('bindir')

systemd_dir = prefix / get_option('libdir') / 'systemd' / 'user'

global_conf = configuration_data()

global_conf.set('bin_install_dir', bindir)

systemd_service_file = configure_file(
  input: './misc/lala-musicbar.service.in',
  output: 'lala-musicbar.service',
  configuration: global_conf,
)

custom_target(
  'lala_musicbar',
  output: 'lala_musicbar',
  build_by_default: true,
  install: true,
  install_dir: bindir,
  console: true,
  command: command,
)

install_data(systemd_service_file, install_dir: systemd_dir)
