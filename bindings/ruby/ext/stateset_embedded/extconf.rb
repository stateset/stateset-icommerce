require 'mkmf'
require 'rb_sys/mkmf'

create_rust_makefile('stateset_embedded/stateset_embedded') do |rust|
  rust.ext_dir = '/../..'
  rust.features = %w[runtime]
end
