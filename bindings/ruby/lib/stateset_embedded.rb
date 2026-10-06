# frozen_string_literal: true

require 'bigdecimal'
require 'json'
require 'time'

begin
  require_relative 'stateset_embedded/stateset_embedded'
rescue LoadError
  require 'stateset_embedded/stateset_embedded'
end

# Ruby binding for the StateSet embedded commerce engine.
#
# Every call goes to the real Rust engine (`stateset-embedded`) through the
# native extension and persists to the SQLite file given to
# {StateSet::Commerce.new} (or to an ephemeral `":memory:"` store).
module StateSet
  VERSION = '1.37.0'
end

require_relative 'stateset_embedded/errors'
require_relative 'stateset_embedded/wire'
require_relative 'stateset_embedded/model'
require_relative 'stateset_embedded/models'
require_relative 'stateset_embedded/apis'
require_relative 'stateset_embedded/commerce'
