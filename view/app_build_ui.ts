import { BuildCatalogEntry, Family, UnitKind } from './types';

export function renderFamilyOptions(
    container: HTMLElement,
    families: Family[],
    onPick: (family: Family) => void,
): void {
    container.innerHTML = '';
    for (const family of families) {
        const btn = document.createElement('button');
        btn.textContent = `Play ${family}`;
        btn.onclick = () => onPick(family);
        container.appendChild(btn);
    }
}

export function renderBuildShop(
    container: HTMLElement,
    catalog: BuildCatalogEntry[],
    onSelect: (unitKind: UnitKind) => void,
): void {
    container.innerHTML = '';
    for (const entry of catalog) {
        const btn = document.createElement('button');
        btn.textContent = `${entry.name} (${entry.cost}g)`;
        btn.onclick = () => onSelect(entry.unit_kind);
        container.appendChild(btn);
    }
}
