import { Editor } from 'https://esm.sh/@tiptap/core@3'
import StarterKit from 'https://esm.sh/@tiptap/starter-kit@3'
import Image from 'https://esm.sh/@tiptap/extension-image@3'
import { Table, TableRow, TableCell, TableHeader } from 'https://esm.sh/@tiptap/extension-table@3'
import TextAlign from 'https://esm.sh/@tiptap/extension-text-align@3'
import { Placeholder, CharacterCount } from 'https://esm.sh/@tiptap/extensions@3'
import { TaskList, TaskItem } from 'https://esm.sh/@tiptap/extension-list@3'
import Subscript from 'https://esm.sh/@tiptap/extension-subscript@3'
import Superscript from 'https://esm.sh/@tiptap/extension-superscript@3'
import { Markdown } from 'https://esm.sh/@tiptap/markdown@3'

const editorElement = document.getElementById('editor');
const contentDataEl = document.getElementById('editor-content-data');
const rawContent = contentDataEl ? contentDataEl.textContent.trim() : '';
const initialContent = rawContent || '';

let editor;

try {
    editor = new Editor({
        element: editorElement,
        extensions: [
            StarterKit.configure({
                link: { openOnClick: false },
            }),
            Image,
            Table.configure({ resizable: true }),
            TableRow,
            TableCell,
            TableHeader,
            TextAlign.configure({ types: ['heading', 'paragraph'] }),
            Placeholder.configure({ placeholder: '开始编写内容...' }),
            TaskList,
            TaskItem.configure({ nested: true }),
            Subscript,
            Superscript,
            CharacterCount,
            Markdown.configure({
                transformPastedText: true,
            }),
        ],
        content: initialContent,
        onUpdate({ editor }) {
            document.getElementById('content-input').value = editor.getHTML();
            updateCharCount(editor);
        },
    });
    const loadingEl = editorElement?.querySelector('.editor-loading');
    if (loadingEl) loadingEl.remove();
} catch (err) {
    console.error('TipTap 编辑器初始化失败:', err);
    if (editorElement) {
        editorElement.innerHTML = '<div class="editor-error">编辑器加载失败，请刷新页面重试</div>';
    }
}

function updateCharCount(ed) {
    const el = document.getElementById('editor-char-count');
    if (!el) return;
    const chars = ed.storage.characterCount.characters();
    const words = ed.storage.characterCount.words();
    el.textContent = words + ' 字 / ' + chars + ' 字符';
}

// 初始化时同步一次
if (editor) {
    document.getElementById('content-input').value = editor.getHTML();
    updateCharCount(editor);
}

// 表单提交时确保最新内容
if (editor) {
    document.querySelectorAll('form').forEach(form => {
        form.addEventListener('submit', () => {
            const input = document.getElementById('content-input');
            if (input) input.value = editor.getHTML();
        });
    });
}

// ── 图片上传 ──

async function uploadImage(file) {
    const formData = new FormData();
    formData.append('file', file);
    const res = await fetch('/admin/api/media/upload', { method: 'POST', body: formData });
    if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error(err.error || '上传失败');
    }
    return await res.json();
}

// 编辑器拖拽上传
if (editorElement && editor) {
    editorElement.addEventListener('drop', async (e) => {
        const files = e.dataTransfer?.files;
        if (files?.length && files[0].type.startsWith('image/')) {
            e.preventDefault();
            try {
                const result = await uploadImage(files[0]);
                editor.chain().focus().setImage({ src: result.url }).run();
            } catch (err) {
                if (typeof showToast === 'function') showToast(err.message, 'error');
            }
        }
    });
    editorElement.addEventListener('dragover', (e) => {
        if (e.dataTransfer?.types?.includes('Files')) e.preventDefault();
    });

    // 编辑器粘贴上传
    editorElement.addEventListener('paste', async (e) => {
        const items = e.clipboardData?.items;
        if (!items) return;
        for (const item of items) {
            if (item.type.startsWith('image/')) {
                e.preventDefault();
                const file = item.getAsFile();
                try {
                    const result = await uploadImage(file);
                    editor.chain().focus().setImage({ src: result.url }).run();
                } catch (err) {
                    if (typeof showToast === 'function') showToast(err.message, 'error');
                }
                break;
            }
        }
    });
}

// 工具栏按钮绑定
const toolbar = document.getElementById('editor-toolbar');
if (toolbar && editor) {
    toolbar.querySelectorAll('button[data-cmd]').forEach(btn => {
        btn.addEventListener('click', () => {
            const cmd = btn.dataset.cmd;
            switch (cmd) {
                case 'bold': editor.chain().focus().toggleBold().run(); break;
                case 'italic': editor.chain().focus().toggleItalic().run(); break;
                case 'underline': editor.chain().focus().toggleUnderline().run(); break;
                case 'strike': editor.chain().focus().toggleStrike().run(); break;
                case 'code': editor.chain().focus().toggleCode().run(); break;
                case 'subscript': editor.chain().focus().toggleSubscript().run(); break;
                case 'superscript': editor.chain().focus().toggleSuperscript().run(); break;
                case 'bulletList': editor.chain().focus().toggleBulletList().run(); break;
                case 'orderedList': editor.chain().focus().toggleOrderedList().run(); break;
                case 'taskList': editor.chain().focus().toggleTaskList().run(); break;
                case 'blockquote': editor.chain().focus().toggleBlockquote().run(); break;
                case 'codeBlock': editor.chain().focus().toggleCodeBlock().run(); break;
                case 'hr': editor.chain().focus().setHorizontalRule().run(); break;
                case 'alignLeft': editor.chain().focus().setTextAlign('left').run(); break;
                case 'alignCenter': editor.chain().focus().setTextAlign('center').run(); break;
                case 'alignRight': editor.chain().focus().setTextAlign('right').run(); break;
                case 'undo': editor.chain().focus().undo().run(); break;
                case 'redo': editor.chain().focus().redo().run(); break;
                case 'link': {
                    openLinkDialog(editor);
                    break;
                }
                case 'image': {
                    openMediaPicker(editor, 'editor');
                    break;
                }
                case 'table': {
                    editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run();
                    break;
                }
            }
        });
    });

    // 标题选择器
    const headingSelect = document.getElementById('tb-heading');
    if (headingSelect) {
        headingSelect.addEventListener('change', () => {
            const val = headingSelect.value;
            if (val === 'p') {
                editor.chain().focus().setParagraph().run();
            } else {
                editor.chain().focus().toggleHeading({ level: parseInt(val) }).run();
            }
        });
    }

    // 更新工具栏激活状态
    editor.on('selectionUpdate', () => updateToolbar());
    editor.on('update', () => updateToolbar());

    function updateToolbar() {
        toolbar.querySelectorAll('button[data-cmd]').forEach(btn => {
            const cmd = btn.dataset.cmd;
            let isActive = false;
            switch (cmd) {
                case 'bold': isActive = editor.isActive('bold'); break;
                case 'italic': isActive = editor.isActive('italic'); break;
                case 'underline': isActive = editor.isActive('underline'); break;
                case 'strike': isActive = editor.isActive('strike'); break;
                case 'code': isActive = editor.isActive('code'); break;
                case 'subscript': isActive = editor.isActive('subscript'); break;
                case 'superscript': isActive = editor.isActive('superscript'); break;
                case 'bulletList': isActive = editor.isActive('bulletList'); break;
                case 'orderedList': isActive = editor.isActive('orderedList'); break;
                case 'taskList': isActive = editor.isActive('taskList'); break;
                case 'blockquote': isActive = editor.isActive('blockquote'); break;
                case 'codeBlock': isActive = editor.isActive('codeBlock'); break;
                case 'alignLeft': isActive = editor.isActive({ textAlign: 'left' }); break;
                case 'alignCenter': isActive = editor.isActive({ textAlign: 'center' }); break;
                case 'alignRight': isActive = editor.isActive({ textAlign: 'right' }); break;
            }
            btn.classList.toggle('active', isActive);
        });

        const headingSelect = document.getElementById('tb-heading');
        if (headingSelect) {
            if (editor.isActive('heading', { level: 1 })) headingSelect.value = '1';
            else if (editor.isActive('heading', { level: 2 })) headingSelect.value = '2';
            else if (editor.isActive('heading', { level: 3 })) headingSelect.value = '3';
            else headingSelect.value = 'p';
        }
    }
}

// ── 链接弹窗 ──

function openLinkDialog(ed) {
    const prevUrl = ed.getAttributes('link').href || '';

    const backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML =
        '<div class="modal" style="max-width:420px;">' +
            '<div class="modal-title">插入链接</div>' +
            '<div class="modal-body">' +
                '<input type="text" id="link-url-input" class="form-input" placeholder="https://..." style="width:100%;">' +
            '</div>' +
            '<div class="modal-actions">' +
                (prevUrl ? '<button class="btn btn-secondary" id="link-remove" style="margin-right:auto;">移除链接</button>' : '') +
                '<button class="btn btn-secondary" id="link-cancel">取消</button>' +
                '<button class="btn btn-primary" id="link-confirm">确定</button>' +
            '</div>' +
        '</div>';
    document.body.appendChild(backdrop);

    const urlInput = document.getElementById('link-url-input');
    urlInput.value = prevUrl;
    urlInput.focus();
    urlInput.select();

    function close() { backdrop.remove(); ed.chain().focus().run(); }

    function confirm() {
        const url = urlInput.value.trim();
        if (url) {
            ed.chain().focus().setLink({ href: url }).run();
        }
        backdrop.remove();
    }

    document.getElementById('link-cancel').onclick = close;
    document.getElementById('link-confirm').onclick = confirm;
    backdrop.onclick = (e) => { if (e.target === backdrop) close(); };
    urlInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') { e.preventDefault(); confirm(); }
        if (e.key === 'Escape') close();
    });

    const removeBtn = document.getElementById('link-remove');
    if (removeBtn) {
        removeBtn.onclick = () => {
            ed.chain().focus().unsetLink().run();
            backdrop.remove();
        };
    }
}

// ── 媒体选择器（支持上传和两种目标：编辑器插入 / 封面图设置） ──

function openMediaPicker(editorRef, target) {
    const backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML =
        '<div class="modal media-picker-modal">' +
            '<div class="modal-title">选择媒体</div>' +
            '<div class="modal-body">' +
                '<div class="media-upload-zone" id="media-upload-zone">点击或拖拽上传图片</div>' +
                '<input type="file" id="media-upload-file" hidden accept="image/*">' +
                '<div class="media-picker-grid" id="media-picker-grid">加载中...</div>' +
            '</div>' +
            '<div class="modal-actions">' +
                '<button class="btn btn-secondary" id="media-picker-cancel">取消</button>' +
                '<div style="flex:1"></div>' +
                '<input type="text" id="media-picker-url" placeholder="或输入图片 URL..." class="form-input" style="width:240px;margin-right:8px;">' +
                '<button class="btn btn-primary" id="media-picker-insert-url">插入 URL</button>' +
            '</div>' +
        '</div>';
    document.body.appendChild(backdrop);

    function insertImage(url) {
        if (target === 'cover') {
            const coverInput = document.getElementById('cover-input');
            if (coverInput) {
                coverInput.value = url;
                coverInput.dispatchEvent(new Event('input'));
            }
        } else if (editorRef) {
            editorRef.chain().focus().setImage({ src: url }).run();
        }
        backdrop.remove();
    }

    document.getElementById('media-picker-cancel').onclick = () => backdrop.remove();
    backdrop.onclick = (e) => { if (e.target === backdrop) backdrop.remove(); };

    document.getElementById('media-picker-insert-url').onclick = () => {
        const url = document.getElementById('media-picker-url').value.trim();
        if (url) insertImage(url);
    };

    // 上传区域交互
    const uploadZone = document.getElementById('media-upload-zone');
    const uploadFile = document.getElementById('media-upload-file');

    uploadZone.onclick = () => uploadFile.click();
    uploadZone.addEventListener('dragover', (e) => { e.preventDefault(); uploadZone.style.borderColor = 'var(--c-brand)'; });
    uploadZone.addEventListener('dragleave', () => { uploadZone.style.borderColor = ''; });
    uploadZone.addEventListener('drop', async (e) => {
        e.preventDefault();
        uploadZone.style.borderColor = '';
        const file = e.dataTransfer?.files?.[0];
        if (file && file.type.startsWith('image/')) {
            await handleUploadInPicker(file, uploadZone, insertImage);
        }
    });

    uploadFile.addEventListener('change', async () => {
        const file = uploadFile.files?.[0];
        if (file) {
            await handleUploadInPicker(file, uploadZone, insertImage);
        }
    });

    async function handleUploadInPicker(file, zone, callback) {
        zone.classList.add('uploading');
        zone.textContent = '上传中...';
        try {
            const result = await uploadImage(file);
            callback(result.url);
        } catch (err) {
            zone.classList.remove('uploading');
            zone.textContent = '上传失败，点击重试';
            if (typeof showToast === 'function') showToast(err.message, 'error');
        }
    }

    // 加载媒体库列表（分页 API，per_page=200 以获取足够多的图片供选择）
    fetch('/admin/api/media?per_page=100')
        .then(r => r.json())
        .then(data => {
            const items = data.items || data;
            const grid = document.getElementById('media-picker-grid');
            if (!items || items.length === 0) {
                grid.innerHTML = '<p style="text-align:center;color:var(--c-text-secondary);">暂无媒体文件</p>';
                return;
            }
            grid.innerHTML = '';
            items.forEach(item => {
                if (!item.url) return;
                const isImage = /\.(jpg|jpeg|png|gif|webp|svg)$/i.test(item.url);
                if (!isImage) return;
                const el = document.createElement('div');
                el.className = 'media-picker-item';
                el.innerHTML = '<img src="' + item.url + '" alt="' + (item.filename || '') + '">' +
                    '<div class="name">' + (item.filename || '') + '</div>';
                el.onclick = () => insertImage(item.url);
                grid.appendChild(el);
            });
        })
        .catch(() => {
            document.getElementById('media-picker-grid').innerHTML =
                '<p style="text-align:center;color:var(--c-danger);">加载媒体失败</p>';
        });
}

// 全局暴露 openMediaPicker 供 HTML onclick 调用
window.openMediaPicker = openMediaPicker;

// ── 分类选择器 ──

const catSelect = document.getElementById('category-select');
const catHidden = document.getElementById('category-input');
if (catSelect && catHidden) {
    fetch('/admin/api/categories').then(r => r.json()).then(cats => {
        cats.forEach(c => {
            const opt = document.createElement('option');
            opt.value = c.name;
            opt.textContent = c.name;
            catSelect.appendChild(opt);
        });
        if (catHidden.value) catSelect.value = catHidden.value;
    }).catch(() => {});

    catSelect.addEventListener('change', () => {
        catHidden.value = catSelect.value;
    });
}

// ── 标签输入组件 ──

const tagContainer = document.getElementById('tag-container');
const tagInputField = document.getElementById('tag-input-field');
const tagsHidden = document.getElementById('tags-input');

if (tagContainer && tagInputField && tagsHidden) {
    let currentTags = tagsHidden.value
        ? tagsHidden.value.split(',').map(t => t.trim()).filter(Boolean)
        : [];
    renderTags();

    function renderTags() {
        tagContainer.querySelectorAll('.tag-badge').forEach(el => el.remove());
        currentTags.forEach((tag, i) => {
            const badge = document.createElement('span');
            badge.className = 'tag-badge';
            badge.innerHTML = tag + ' <span class="tag-remove" data-index="' + i + '">&times;</span>';
            tagContainer.insertBefore(badge, tagInputField);
        });
        tagsHidden.value = currentTags.join(',');
    }

    tagInputField.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ',') {
            e.preventDefault();
            const val = tagInputField.value.trim();
            if (val && !currentTags.includes(val)) {
                currentTags.push(val);
                renderTags();
            }
            tagInputField.value = '';
        }
        if (e.key === 'Backspace' && !tagInputField.value && currentTags.length) {
            currentTags.pop();
            renderTags();
        }
    });

    tagContainer.addEventListener('click', (e) => {
        const removeBtn = e.target.closest('.tag-remove');
        if (removeBtn) {
            const index = parseInt(removeBtn.dataset.index);
            currentTags.splice(index, 1);
            renderTags();
        } else {
            tagInputField.focus();
        }
    });
}

// ── 封面图预览 ──

const coverInput = document.getElementById('cover-input');
const coverPreview = document.getElementById('cover-preview');
const coverPreviewImg = document.getElementById('cover-preview-img');

if (coverInput && coverPreview && coverPreviewImg) {
    coverInput.addEventListener('input', () => {
        const url = coverInput.value.trim();
        if (url) {
            coverPreviewImg.src = url;
            coverPreview.style.display = '';
        } else {
            coverPreview.style.display = 'none';
        }
    });
}

// ── 自动保存草稿（仅新建文章时启用）──

const postForm = document.getElementById('post-form');
const isNewPost = postForm && postForm.getAttribute('action') === '/admin/posts';

if (isNewPost && editor) {
    let draftId = null;
    let saveTimer = null;
    let isSaving = false;
    const saveStatusEl = document.getElementById('auto-save-status');

    function setStatus(msg, isError) {
        if (!saveStatusEl) return;
        saveStatusEl.textContent = msg;
        saveStatusEl.style.color = isError ? 'var(--c-danger)' : 'var(--c-text-secondary)';
    }

    async function doAutosave() {
        if (isSaving) return;
        const title = document.querySelector('input[name="title"]')?.value || '';
        const content = document.getElementById('content-input')?.value || '';
        if (!title && !content) return;

        isSaving = true;
        setStatus('保存中...');
        try {
            const body = {
                title: title || '(无标题草稿)',
                content,
                tags: document.getElementById('tags-input')?.value || '',
                category: document.getElementById('category-input')?.value || '',
                cover_image: document.getElementById('cover-input')?.value || '',
                excerpt: document.querySelector('textarea[name="excerpt"]')?.value || '',
                slug: document.querySelector('input[name="slug"]')?.value || '',
            };

            let resp;
            if (!draftId) {
                resp = await fetch('/admin/posts/autosave', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(body),
                });
                if (resp.ok) {
                    const data = await resp.json();
                    draftId = data.id;
                    if (postForm) postForm.setAttribute('action', '/admin/posts/' + draftId);
                    setStatus('草稿已保存');
                } else {
                    setStatus('保存失败', true);
                }
            } else {
                resp = await fetch('/admin/posts/' + draftId + '/autosave', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(body),
                });
                if (resp.ok) {
                    setStatus('草稿已保存 ' + new Date().toLocaleTimeString());
                } else {
                    setStatus('保存失败', true);
                }
            }
        } catch {
            setStatus('网络错误，保存失败', true);
        } finally {
            isSaving = false;
        }
    }

    function scheduleAutosave() {
        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = setTimeout(doAutosave, 3000);
    }

    document.querySelector('input[name="title"]')?.addEventListener('input', scheduleAutosave);
    editor.on('update', scheduleAutosave);
    setInterval(doAutosave, 30000);
}
