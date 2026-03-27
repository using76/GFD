import React from 'react';
import { Form, Input, InputNumber, Select, Checkbox } from 'antd';

export type PropertyType = 'number' | 'string' | 'select' | 'checkbox' | 'vector3';

export interface PropertyField {
  key: string;
  label: string;
  type: PropertyType;
  options?: { label: string; value: string | number }[];
  min?: number;
  max?: number;
  step?: number;
  precision?: number;
}

interface PropertyGridProps {
  fields: PropertyField[];
  values: Record<string, unknown>;
  onChange: (key: string, value: unknown) => void;
  title?: string;
}

const PropertyGrid: React.FC<PropertyGridProps> = ({
  fields,
  values,
  onChange,
  title,
}) => {
  return (
    <div style={{ padding: 12 }}>
      {title && (
        <div
          style={{
            fontWeight: 600,
            marginBottom: 12,
            fontSize: 14,
            borderBottom: '1px solid #303030',
            paddingBottom: 8,
          }}
        >
          {title}
        </div>
      )}
      <Form layout="vertical" size="small">
        {fields.map((field) => {
          const val = values[field.key];
          switch (field.type) {
            case 'number':
              return (
                <Form.Item key={field.key} label={field.label}>
                  <InputNumber
                    value={val as number}
                    min={field.min}
                    max={field.max}
                    step={field.step ?? 0.01}
                    precision={field.precision}
                    onChange={(v) => onChange(field.key, v)}
                    style={{ width: '100%' }}
                  />
                </Form.Item>
              );
            case 'string':
              return (
                <Form.Item key={field.key} label={field.label}>
                  <Input
                    value={val as string}
                    onChange={(e) => onChange(field.key, e.target.value)}
                  />
                </Form.Item>
              );
            case 'select':
              return (
                <Form.Item key={field.key} label={field.label}>
                  <Select
                    value={val as string | number}
                    options={field.options}
                    onChange={(v) => onChange(field.key, v)}
                  />
                </Form.Item>
              );
            case 'checkbox':
              return (
                <Form.Item key={field.key} valuePropName="checked">
                  <Checkbox
                    checked={val as boolean}
                    onChange={(e) => onChange(field.key, e.target.checked)}
                  >
                    {field.label}
                  </Checkbox>
                </Form.Item>
              );
            case 'vector3': {
              const vec = (val as [number, number, number]) ?? [0, 0, 0];
              return (
                <Form.Item key={field.key} label={field.label}>
                  <div style={{ display: 'flex', gap: 4 }}>
                    {(['X', 'Y', 'Z'] as const).map((axis, i) => (
                      <InputNumber
                        key={axis}
                        value={vec[i]}
                        step={field.step ?? 0.1}
                        placeholder={axis}
                        style={{ flex: 1 }}
                        onChange={(v) => {
                          const next = [...vec] as [number, number, number];
                          next[i] = v ?? 0;
                          onChange(field.key, next);
                        }}
                      />
                    ))}
                  </div>
                </Form.Item>
              );
            }
            default:
              return null;
          }
        })}
      </Form>
    </div>
  );
};

export default PropertyGrid;
