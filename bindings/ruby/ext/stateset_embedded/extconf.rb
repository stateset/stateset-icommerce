# frozen_string_literal: true

require 'mkmf'
require 'rb_sys/mkmf'

# Cargo.toml lives at the gem root (two levels up), next to src/.
create_rust_makefile('stateset_embedded/stateset_embedded') do |rust|
  rust.ext_dir = '/../..' # appended to this file's dir by rb_sys
  rust.features = %w[runtime]
end
