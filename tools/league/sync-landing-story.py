"""Keep the static, no-JavaScript landing copy in sync with the canonical story."""
import html
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
PAGE = ROOT/'web/games/league/index.html'
story = json.loads((PAGE.parent/'story.json').read_text(encoding='utf-8'))
source = PAGE.read_text(encoding='utf-8')
esc = html.escape


def replace(pattern, value):
    global source
    source, count = re.subn(pattern, lambda match: value(match) if callable(value) else value, source, flags=re.S)
    assert count == 1, f'Expected one static story anchor: {pattern}'


replace(r'(<h1 id="hero-title">).*?(</h1>)', lambda m: m[1]+esc(story['campaign']['headline'])+m[2])
replace(r'<p class="tagline" id="campaign-tagline"[^>]*>.*?</p>', '<p class="tagline" id="campaign-tagline">'+esc(story['campaign']['tagline'])+'</p>')
replace(r'(<p class="lede" id="hero-lede">).*?(</p>)', lambda m: m[1]+esc(story['campaign']['description'])+m[2])
replace(r'(<h3 id="world-title">).*?(</h3>)', lambda m: m[1]+esc(story['world']['title'])+m[2])
replace(r'<p class="world-sub" id="world-subtitle"[^>]*>.*?</p>', '<p class="world-sub" id="world-subtitle">'+esc(story['world']['subtitle'])+'</p>')
replace(r'(<div class="world-para" id="world-para">).*?(</div>)', lambda m: m[1]+'\n'+''.join('        <p>'+esc(p)+'</p>\n' for p in story['world']['paragraphs'])+'      '+m[2])

for champion in story['champions']:
    pattern = r'<button class="champ fade"[^>]*data-key="'+champion['id']+r'".*?</button>'
    def card(match):
        value = match[0]
        fields = {'name':champion['name'], 'title':champion['epithet'], 'desc':champion['hook'],
                  'origin':'\n\n'.join(champion['origin']), 'motive':champion['motivation'],
                  'flaw':champion['flaw'], 'relationship':champion['bond']['text']}
        for key, text in fields.items():
            value, count = re.subn(r'data-'+key+r'="[^"]*"', lambda m: 'data-'+key+'="'+esc(text, quote=True).replace('\n','&#10;')+'"', value)
            assert count == 1, f'Missing fallback field {champion["id"]}.{key}'
        for cls, text in [('c-name',champion['name']), ('c-title',champion['epithet']), ('c-desc',champion['hook'])]:
            value = re.sub(r'(<span class="'+cls+r'">).*?(</span>)', lambda m: m[1]+esc(text)+m[2], value, flags=re.S)
        value = re.sub(r'alt="[^"]*"', lambda m: 'alt="'+esc(champion['name']+', '+champion['epithet'], quote=True)+'"', value)
        return value
    replace(pattern, card)

chapters = []
for chapter in story['chapters']:
    paragraphs = ''.join('          <p'+(' class="hook"' if i == len(chapter['body'])-1 else '')+'>'+esc(p)+'</p>\n' for i,p in enumerate(chapter['body']))
    chapters.append(f'''      <li class="chapter fade" id="{esc(chapter['id'], quote=True)}">
        <span class="cnum" aria-hidden="true">{chapter['number']:02d}</span>
        <div class="cbody">
          <img class="cimg" src="{esc(chapter['image'], quote=True)}" alt="" loading="lazy">
          <h3>{esc(chapter['title'])}</h3>
          <p class="csub">{esc(chapter['subtitle'])}</p>
{paragraphs}        </div>
      </li>''')
replace(r'<ol class="chapters" id="chapters">.*?</ol>', '<ol class="chapters" id="chapters">\n'+'\n'.join(chapters)+'\n    </ol>')
features = []
for feature, icon in zip(story['features'], ['dagger','swarm-q','crystal','coin','storm','aegis']):
    features.append(f'''      <li class="feature fade">
        <svg class="ficon" viewBox="0 0 64 64" aria-hidden="true"><use href="./v2/art/icons.svg#{icon}"></use></svg>
        <h3>{esc(feature['title'])}</h3>
        <p>{esc(feature['text'])}</p>
      </li>''')
replace(r'<ul class="features">.*?</ul>', '<ul class="features">\n'+'\n'.join(features)+'\n    </ul>')
source = source.replace('Draft copy below is the no-story.json fallback; Fable\'s story.json\n       rewrites these fields at runtime. Do not quote this text as canon.', 'Canonical story fallback, synchronized by tools/league/sync-landing-story.py.\n       story.json also updates the reader at runtime.')
PAGE.write_text(source, encoding='utf-8', newline='\n')
print('Synchronized hero, world, five origins, three complete chapters and features.')
