# frozen_string_literal: true

module StateSet
  # The JSON contract with the native extension (see `src/dispatch.rs`).
  #
  # Money and quantities cross as exact decimal STRINGS. `Float` is refused
  # everywhere in the arguments: `0.1 + 0.2` is not `0.3` in binary floating
  # point, and a commerce engine must never silently take a rounded amount.
  # Pass `BigDecimal`, `Integer`, or a decimal `String` instead.
  module Wire
    module_function

    # Convert a Ruby argument tree into JSON-safe values.
    def encode(value, path = 'arguments')
      case value
      when Hash
        value.each_with_object({}) do |(key, val), out|
          out[key.to_s] = encode(val, "#{path}.#{key}")
        end
      when Array
        value.each_with_index.map { |val, i| encode(val, "#{path}[#{i}]") }
      when BigDecimal
        raise ValidationError.new("#{path}: #{value} is not a finite decimal", code: 'binding.invalid_argument', status: 400) unless value.finite?

        value.to_s('F')
      when Float
        raise ValidationError.new(
          "#{path}: Float #{value} refused -- money and quantities must be exact; " \
          "pass BigDecimal('#{value}') or the String '#{value}'",
          code: 'binding.float_refused', status: 400
        )
      when Rational
        raise ValidationError.new("#{path}: Rational refused; pass a BigDecimal or decimal String",
                                  code: 'binding.invalid_argument', status: 400)
      when Symbol
        value.to_s
      when Time
        value.utc.iso8601(6)
      when String, Integer, true, false, nil
        value
      else
        raise ValidationError.new("#{path}: unsupported argument type #{value.class}",
                                  code: 'binding.invalid_argument', status: 400)
      end
    end

    # Call the engine and return the decoded `value`, raising the matching
    # {StateSet::Error} subclass on failure.
    def call(native, op, args = {})
      reply = native.call(op, JSON.generate(encode(args)))
      unwrap(reply)
    end

    # Decode a native envelope.
    def unwrap(reply)
      envelope = JSON.parse(reply, symbolize_names: true)
      return envelope[:value] if envelope[:ok]

      error = envelope[:error] || {}
      klass = ERROR_CLASSES.fetch(error[:kind].to_s, Error)
      raise klass.new(error[:message], code: error[:code], status: error[:status])
    end
  end
end
