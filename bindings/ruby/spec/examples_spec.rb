# frozen_string_literal: true

# Runs every script in examples/ruby against the built extension so the
# published examples cannot drift from the real API again.

require 'spec_helper'
require 'open3'
require 'rbconfig'

RSpec.describe 'examples/ruby' do
  lib = File.expand_path('../lib', __dir__)
  Dir[File.expand_path('../../../examples/ruby/*.rb', __dir__)].sort.each do |path|
    it "runs #{File.basename(path)} cleanly" do
      out, err, status = Open3.capture3(RbConfig.ruby, '-I', lib, path)
      expect(status).to be_success, "#{File.basename(path)} failed:\n#{err}\n#{out}"
      expect(out).not_to be_empty
    end
  end
end
