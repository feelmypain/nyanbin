import { createAPI } from '../shared/api.js'
import { parseNoteReference } from '../shared/protocol.js'

export async function deleteNoteByLink(input: URL | string, deleteToken: string): Promise<void> {
  const link = parseNoteReference(input.toString())
  const api = createAPI({ server: link.server })
  await api.deleteNote(link.id, deleteToken)
}
