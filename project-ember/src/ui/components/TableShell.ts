/**
 * Builds an empty `<table>` with a `<thead>` header row from the given column labels and an empty `<tbody>`, returning both so the caller fills in rows.
 * Styling is left to the caller via `className`; the shell adds no classes of its own.
 * DOM: `table > (thead > tr > th*) + tbody`.
 */
export function createTableShell(
  columns: string[],
  className?: string,
): { table: HTMLTableElement; tbody: HTMLTableSectionElement } {
  const table = document.createElement('table');
  if (className) {
    table.className = className;
  }

  const thead = document.createElement('thead');
  const headerRow = document.createElement('tr');
  for (const label of columns) {
    const th = document.createElement('th');
    th.textContent = label;
    headerRow.appendChild(th);
  }
  thead.appendChild(headerRow);
  table.appendChild(thead);

  const tbody = document.createElement('tbody');
  table.appendChild(tbody);

  return { table, tbody };
}
