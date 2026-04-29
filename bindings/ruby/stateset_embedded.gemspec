Gem::Specification.new do |s|
  s.name        = 'stateset_embedded'
  s.version      = '1.0.0'
  s.summary     = 'Local-first commerce engine for Ruby'
  s.description = 'Embedded commerce operations with SQLite storage. Provides a complete commerce API including customers, orders, products, inventory, returns, carts, analytics, and more.'
  s.authors     = ['StateSet']
  s.email       = 'hello@stateset.io'
  s.homepage    = 'https://github.com/stateset/stateset-icommerce'
  s.license     = 'MIT'

  s.files       = Dir['lib/**/*.rb', 'ext/**/*', 'Cargo.toml', 'src/**/*.rs', 'extconf.rb']
  s.extensions  = ['ext/stateset_embedded/extconf.rb']

  s.required_ruby_version = '>= 3.0'
  s.required_rubygems_version = '>= 3.3.11'

  s.add_development_dependency 'rake', '~> 13.0'
  s.add_development_dependency 'rake-compiler', '~> 1.2'
  s.add_development_dependency 'rb_sys', '0.9.50'
  s.add_development_dependency 'rspec', '~> 3.12'

  s.metadata = {
    'homepage_uri' => 'https://github.com/stateset/stateset-icommerce',
    'source_code_uri' => 'https://github.com/stateset/stateset-icommerce/tree/main/bindings/ruby',
    'documentation_uri' => 'https://github.com/stateset/stateset-icommerce#ruby-bindings',
    'rubygems_mfa_required' => 'true'
  }
end
