export function castleView(state) {
  const quest = state.quest;
  if (!quest || state.stage < 5) return null;
  const sequence = Math.max(0, Math.min(3, Number(quest.sequence) || 0));
  const enemy = state.enemy;
  return {
    objective: state.objective,
    seals: `Ancient seals · ${sequence} / 3 awakened`,
    journal: quest.journal || [],
    inventory: `${quest.seal ? 'Castellan’s crown recovered' : 'No crown'} · ${3 - Math.min(3, quest.shrinesUsed || 0)} healing shrines remain`,
    enemy: enemy ? {
      name: enemy.name,
      health: Math.max(0, enemy.health),
      maximum: enemy.maxHealth,
      boss: !!enemy.boss,
      cue: enemy.cue,
      danger: !!enemy.unblockable,
    } : null,
    escaped: state.finished,
    stats: `${quest.defeated || 0} foes defeated · ${state.exploration?.visited || 0} places discovered`,
  };
}

export function renderCastle(state, get) {
  const view = castleView(state);
  get('seal-progress').hidden = !view;
  get('journal-hint').hidden = !view;
  get('enemy-status').hidden = !view?.enemy || state.finished;
  if (!view) return;
  get('seal-progress').textContent = view.seals;
  get('journal-objective').textContent = view.objective;
  get('journal-inventory').textContent = view.inventory;
  // Only rebuild the journal when discoveries change; never interpret story text as HTML.
  const entries = get('journal-entries'), key = JSON.stringify(view.journal);
  if (entries.dataset.entries !== key) {
    entries.dataset.entries = key;
    entries.replaceChildren(...view.journal.map(text => {
      const li = entries.ownerDocument.createElement('li'); li.textContent = text; return li;
    }));
  }
  if (view.enemy) {
    const enemy = view.enemy;
    get('enemy-status').dataset.boss = String(enemy.boss);
    get('enemy-status').dataset.danger = String(enemy.danger);
    get('enemy-name').textContent = enemy.name;
    get('enemy-health').max = enemy.maximum;
    get('enemy-health').value = enemy.health;
    get('enemy-cue').textContent = enemy.cue;
  }
  get('escape-stats').textContent = view.stats;
}
