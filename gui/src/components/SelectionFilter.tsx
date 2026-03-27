import React from 'react';
import { Select } from 'antd';
import { useAppStore } from '../store/useAppStore';
import type { SelectionFilterType } from '../store/useAppStore';

const options: { value: SelectionFilterType; label: string }[] = [
  { value: 'face', label: 'Face' },
  { value: 'edge', label: 'Edge' },
  { value: 'vertex', label: 'Vertex' },
  { value: 'body', label: 'Body' },
  { value: 'component', label: 'Component' },
];

const SelectionFilter: React.FC = () => {
  const selectionFilter = useAppStore((s) => s.selectionFilter);
  const setSelectionFilter = useAppStore((s) => s.setSelectionFilter);

  return (
    <Select
      value={selectionFilter}
      onChange={setSelectionFilter}
      options={options}
      size="small"
      style={{ width: 110, fontSize: 11 }}
      popupMatchSelectWidth={false}
    />
  );
};

export default SelectionFilter;
