import type { ProjectDirectoryListing } from '$lib/generated/feedback'

const folders: Record<string, string[]> = {
  'C:/': ['Projects', 'Users'],
  'C:/Users': ['A'],
  'C:/Users/A': ['Desktop', 'Documents'],
  'C:/Users/A/Desktop': ['codeg', 'rambledesk'],
  'C:/Users/A/Documents': [],
  'C:/Users/A/Desktop/codeg': ['src'],
  'C:/Users/A/Desktop/rambledesk': ['apps', 'crates', 'playground'],
  'C:/Projects': ['rambledesk', 'welcome'],
  'C:/Projects/rambledesk': ['apps', 'crates', 'playground'],
  'C:/Projects/welcome': ['src'],
  'D:/': ['Work', 'Archive'],
  'D:/Work': ['welcome'],
  'D:/Archive': ['rambledesk'],
}

export async function browsePreviewDirectories(input: { path: string | null }): Promise<ProjectDirectoryListing> {
  await new Promise(resolve => setTimeout(resolve, 180))
  let path = input.path?.replace(/\\/g, '/').replace(/\/$/, '') || 'C:/Users/A'
  if (/^[A-Z]:$/i.test(path)) path += '/'
  const parent_path = /^[A-Z]:\/$/i.test(path) ? null : path.slice(0, path.lastIndexOf('/') + (path.lastIndexOf('/') === 2 ? 1 : 0))
  if (!(path in folders) && !folders[parent_path ?? '']?.includes(path.split('/').pop()!)) {
    throw { code: 'DIRECTORY_NOT_FOUND', message: 'Fixture directory was not found.', retryable: false }
  }
  return {
    path, parent_path, home_path: 'C:/Users/A',
    roots: [{ name: 'C:', path: 'C:/' }, { name: 'D:', path: 'D:/' }],
    directories: (folders[path] ?? []).map(name => ({ name, path: `${path.replace(/\/$/, '')}/${name}` })),
  }
}
