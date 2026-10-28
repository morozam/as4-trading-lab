export function profileValues(values: string[], hourCount: number, kind: string): number[] {
  if (!hourCount || values.length !== hourCount) throw new Error(`Complete all ${hourCount} hourly intervals before saving.`);
  if (kind === 'PRODOC') return Array(hourCount).fill(0);
  return values.map((value, index) => {
    const quantity = Number(value);
    if (!value.trim() || !Number.isSafeInteger(quantity) || quantity < 0) {
      throw new Error(`Hour ${index + 1}: enter a non-negative whole quantity in kWh/h.`);
    }
    return quantity;
  });
}
