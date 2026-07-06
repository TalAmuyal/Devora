import assert from 'node:assert';
import { AppDriver } from './app-driver';

/** Assert each item of an open dropdown popup is the top element at its own center — catches ancestor overflow clipping and occlusion, which existence checks miss */
export async function assertDropdownItemsHitTestable(
  driver: AppDriver,
  popupSelector: string,
): Promise<void> {
  const itemSelector = `${popupSelector} .dropdown-item`;
  const blocked: string[] = await driver.eval(`
    const items = Array.from(document.querySelectorAll(${JSON.stringify(itemSelector)}));
    if (items.length === 0) throw new Error('No dropdown items under: ' + ${JSON.stringify(popupSelector)});
    return items
      .filter((item) => {
        const rect = item.getBoundingClientRect();
        const hit = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
        return !item.contains(hit);
      })
      .map((item) => item.textContent.trim());
  `);
  assert.deepStrictEqual(
    blocked,
    [],
    `Dropdown items are not clickable at their on-screen position: ${blocked.join(', ')}`,
  );
}
