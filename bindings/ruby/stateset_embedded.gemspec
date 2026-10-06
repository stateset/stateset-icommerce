Gem::Specification.new do |s|
  s.name        = 'stateset_embedded'
  s.version     = '1.37.0'
  s.summary     = 'Embedded commerce engine for Ruby (native binding to the StateSet Rust engine)'
  s.description = 'Ruby binding for the StateSet embedded commerce engine: customers, products, inventory, ' \
                  'carts and checkout, orders, payments and refunds, returns and shipments, persisted to a ' \
                  'local SQLite file. Money is exact (BigDecimal), never Float.'
  s.authors     = ['StateSet']
  s.email       = 'hello@stateset.io'
  s.homepage    = 'https://github.com/stateset/stateset-icommerce'
  s.licenses    = ['MIT', 'Apache-2.0']

  s.files       = Dir['lib/**/*.rb', 'ext/**/*', 'Cargo.toml', 'Cargo.lock', 'src/**/*.rs', 'README.md']
  s.extensions  = ['ext/stateset_embedded/extconf.rb']

  s.required_ruby_version = '>= 3.0'
  s.required_rubygems_version = '>= 3.3.11'

  # bigdecimal is a bundled (not default) gem from Ruby 3.4.
  s.add_dependency 'bigdecimal', '>= 3.1'
  s.add_dependency 'rb_sys', '~> 0.9'

  s.add_development_dependency 'rake', '~> 13.0'
  s.add_development_dependency 'rake-compiler', '~> 1.2'
  s.add_development_dependency 'rspec', '~> 3.12'

  s.metadata = {
    'homepage_uri' => 'https://github.com/stateset/stateset-icommerce',
    'source_code_uri' => 'https://github.com/stateset/stateset-icommerce/tree/master/bindings/ruby',
    'documentation_uri' => 'https://github.com/stateset/stateset-icommerce/blob/master/docs/src/api/ruby.md',
    'rubygems_mfa_required' => 'true'
  }
end
