# frozen_string_literal: true

module StateSet
  # Immutable value object built from an engine record.
  #
  # Every field the engine returns is readable as a method (`order.status`)
  # and with `[]` (`order[:status]`); {#to_h} gives the whole record. The
  # field set is whatever the engine serializes, so it cannot drift from the
  # engine. Subclasses only declare how to TYPE fields:
  #
  # * `decimal` fields become `BigDecimal` (money, quantities) -- never Float;
  # * `nested` fields become arrays/instances of another model;
  # * fields ending in `_at` (plus any declared `time` field) become `Time`.
  class Model
    class << self
      def decimal(*names)
        decimal_fields.concat(names)
      end

      def nested(mapping)
        nested_fields.merge!(mapping)
      end

      def time(*names)
        time_fields.concat(names)
      end

      def decimal_fields
        @decimal_fields ||= []
      end

      def nested_fields
        @nested_fields ||= {}
      end

      def time_fields
        @time_fields ||= []
      end

      # Build from a decoded engine record; `nil` stays `nil`.
      def from(record)
        record.nil? ? nil : new(record)
      end

      def from_list(records)
        Array(records).map { |record| new(record) }
      end

      private

      def define_reader(key)
        return if method_defined?(key) || Object.method_defined?(key)

        define_method(key) { @attributes[key] }
      end
    end

    def initialize(record)
      raise ArgumentError, "expected a Hash, got #{record.class}" unless record.is_a?(Hash)

      @attributes = record.each_with_object({}) do |(key, value), out|
        key = key.to_sym
        out[key] = coerce(key, value)
        self.class.send(:define_reader, key)
      end.freeze
      freeze
    end

    def [](key)
      @attributes[key.to_sym]
    end

    def key?(key)
      @attributes.key?(key.to_sym)
    end

    def to_h
      @attributes.transform_values do |value|
        case value
        when Model then value.to_h
        when Array then value.map { |v| v.is_a?(Model) ? v.to_h : v }
        else value
        end
      end
    end

    def ==(other)
      other.class == self.class && other.to_h == to_h
    end
    alias eql? ==

    def hash
      [self.class, @attributes].hash
    end

    def inspect
      id = @attributes[:id]
      "#<#{self.class.name}#{id ? " id=#{id}" : ''}>"
    end

    private

    def coerce(key, value)
      return nil if value.nil?

      if (klass = self.class.nested_fields[key])
        value.is_a?(Array) ? klass.from_list(value) : klass.from(value)
      elsif self.class.decimal_fields.include?(key)
        BigDecimal(value.to_s)
      elsif value.is_a?(String) && (key.to_s.end_with?('_at') || self.class.time_fields.include?(key))
        parse_time(value)
      else
        deep_freeze(value)
      end
    end

    def parse_time(value)
      Time.iso8601(value)
    rescue ArgumentError
      value.freeze
    end

    def deep_freeze(value)
      case value
      when Hash then value.transform_values { |v| deep_freeze(v) }.freeze
      when Array then value.map { |v| deep_freeze(v) }.freeze
      else value.freeze
      end
    end
  end
end
