import type { ImageUpload, NoteImage } from './domain'

export const maxNoteImages = 9
export const maxImageBytes = 2 * 1024 * 1024

export interface PreparedImage {
  image: NoteImage
  upload: ImageUpload
  url: string
}

export async function prepareImage(file: File): Promise<PreparedImage> {
  if (!/^image\/(png|jpeg|webp|gif|bmp|avif)$/.test(file.type)) throw new Error('请选择 PNG、JPG、WebP、GIF、BMP 或 AVIF 图片')
  if (file.size > 20 * 1024 * 1024) throw new Error('单张图片不能超过 20 MB')
  const source = URL.createObjectURL(file)
  try {
    const image = new Image()
    image.src = source
    await image.decode().catch(() => { throw new Error(`无法读取图片：${file.name}`) })
    const scale = Math.min(1, 2048 / Math.max(image.naturalWidth, image.naturalHeight))
    const canvas = document.createElement('canvas')
    canvas.width = Math.max(1, Math.round(image.naturalWidth * scale))
    canvas.height = Math.max(1, Math.round(image.naturalHeight * scale))
    const context = canvas.getContext('2d')
    if (!context) throw new Error('无法处理图片，请重试')
    context.drawImage(image, 0, 0, canvas.width, canvas.height)
    let url = canvas.toDataURL('image/webp', 0.88)
    if (url.length * 0.75 > maxImageBytes) url = canvas.toDataURL('image/webp', 0.7)
    const [header, data] = url.split(',')
    if (!data || data.length * 0.75 > maxImageBytes) throw new Error('图片处理后仍超过 2 MB，请选择较小图片')
    const mime = header!.slice(5, header!.indexOf(';'))
    const id = crypto.randomUUID()
    return { image: { id, name: (file.name || '粘贴图片').slice(0, 200), mime, width: canvas.width, height: canvas.height }, upload: { id, data }, url }
  } finally { URL.revokeObjectURL(source) }
}

let previewDatabase: Promise<IDBDatabase> | undefined
function openImageDatabase() {
  return previewDatabase ||= new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open('riji-preview-images', 1)
    request.onupgradeneeded = () => request.result.createObjectStore('images', { keyPath: 'id' })
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => { previewDatabase = undefined; reject(new Error('无法打开图片存储')) }
  })
}

export interface StoredImage { image: NoteImage; data: string }

export async function readPreviewImage(id: string): Promise<StoredImage> {
  const database = await openImageDatabase()
  return new Promise((resolve, reject) => {
    const request = database.transaction('images').objectStore('images').get(id)
    request.onsuccess = () => request.result ? resolve({ image: request.result.image, data: request.result.data }) : reject(new Error('图片不存在'))
    request.onerror = () => reject(new Error('无法读取图片'))
  })
}

export async function writePreviewImages(images: StoredImage[]) {
  if (!images.length) return
  const database = await openImageDatabase()
  await new Promise<void>((resolve, reject) => {
    const transaction = database.transaction('images', 'readwrite')
    images.forEach(asset => transaction.objectStore('images').put({ id: asset.image.id, image: { ...asset.image }, data: asset.data }))
    transaction.oncomplete = () => resolve()
    transaction.onerror = () => reject(new Error('无法保存图片，请检查可用存储空间'))
    transaction.onabort = () => reject(new Error('图片保存已取消'))
  })
}

export async function prunePreviewImages(ids: Set<string>) {
  const database = await openImageDatabase()
  await new Promise<void>((resolve, reject) => {
    const transaction = database.transaction('images', 'readwrite')
    const request = transaction.objectStore('images').openKeyCursor()
    request.onsuccess = () => {
      const cursor = request.result
      if (!cursor) return
      if (!ids.has(String(cursor.primaryKey))) transaction.objectStore('images').delete(cursor.primaryKey)
      cursor.continue()
    }
    transaction.oncomplete = () => resolve()
    transaction.onerror = () => reject(new Error('无法清理已删除的图片'))
  })
}
