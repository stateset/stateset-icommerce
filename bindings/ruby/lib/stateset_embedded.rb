# frozen_string_literal: true

begin
  require_relative 'stateset_embedded/stateset_embedded'
rescue LoadError
  require 'stateset_embedded/stateset_embedded'
end

module StateSet
  VERSION = '0.7.5'
end
