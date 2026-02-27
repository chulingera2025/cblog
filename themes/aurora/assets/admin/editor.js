import { Editor } from 'https://esm.sh/@tiptap/core@2'
import StarterKit from 'https://esm.sh/@tiptap/starter-kit@2'
import Link from 'https://esm.sh/@tiptap/extension-link@2'
import Image from 'https://esm.sh/@tiptap/extension-image@2'
import Table from 'https://esm.sh/@tiptap/extension-table@2'
import TableRow from 'https://esm.sh/@tiptap/extension-table-row@2'
import TableCell from 'https://esm.sh/@tiptap/extension-table-cell@2'
import TableHeader from 'https://esm.sh/@tiptap/extension-table-header@2'
import Underline from 'https://esm.sh/@tiptap/extension-underline@2'
import TextAlign from 'https://esm.sh/@tiptap/extension-text-align@2'
import Placeholder from 'https://esm.sh/@tiptap/extension-placeholder@2'
import TextStyle from 'https://esm.sh/@tiptap/extension-text-style@2'
import Color from 'https://esm.sh/@tiptap/extension-color@2'
import Highlight from 'https://esm.sh/@tiptap/extension-highlight@2'
import TaskList from 'https://esm.sh/@tiptap/extension-task-list@2'
import TaskItem from 'https://esm.sh/@tiptap/extension-task-item@2'

const contentDataEl = document.getElementById('editor-content-data');
const initialContent = contentDataEl ? JSON.parse(contentDataEl.textContent) : '';

const editor = new Editor({
    element: document.getElementById('editor'),
    extensions: [
        StarterKit,
        Link.configure({ openOnClick: false }),
        Image,
        Table.configure({ resizable: true }),
        TableRow,
        TableCell,
        TableHeader,
        Underline,
        TextAlign.configure({ types: ['heading', 'paragraph'] }),
        Placeholder.configure({ placeholder: '开始编写内容...' }),
        TextStyle,
        Color,
        Highlight.configure({ multicolor: true }),
        TaskList,
        TaskItem.configure({ nested: true }),
    ],
    content: initialContent,
    onUpdate({ editor }) {
        document.getElementById('content-input').value = editor.getHTML();
    },
});

// 初始化时同步一次
document.getElementById('content-input').value = editor.getHTML();

// 表单提交时确保最新内容
document.querySelectorAll('form').forEach(form => {
    form.addEventListener('submit', () => {
        const input = document.getElementById('content-input');
        if (input) input.value = editor.getHTML();
    });
});

// 工具栏按钮绑定
const toolbar = document.getElementById('editor-toolbar');
if (toolbar) {
    toolbar.querySelectorAll('button[data-cmd]').forEach(btn => {
        btn.addEventListener('click', () => {
            const cmd = btn.dataset.cmd;
            switch (cmd) {
                case 'bold': editor.chain().focus().toggleBold().run(); break;
                case 'italic': editor.chain().focus().toggleItalic().run(); break;
                case 'underline': editor.chain().focus().toggleUnderline().run(); break;
                case 'strike': editor.chain().focus().toggleStrike().run(); break;
                case 'code': editor.chain().focus().toggleCode().run(); break;
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
                    const url = prompt('输入链接 URL:');
                    if (url) editor.chain().focus().setLink({ href: url }).run();
                    break;
                }
                case 'image': {
                    openMediaPicker(editor);
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

        // 更新标题选择器
        const headingSelect = document.getElementById('tb-heading');
        if (headingSelect) {
            if (editor.isActive('heading', { level: 1 })) headingSelect.value = '1';
            else if (editor.isActive('heading', { level: 2 })) headingSelect.value = '2';
            else if (editor.isActive('heading', { level: 3 })) headingSelect.value = '3';
            else headingSelect.value = 'p';
        }
    }
}

// 媒体选择器
function openMediaPicker(editor) {
    const backdrop = document.createElement('div');
    backdrop.className = 'modal-backdrop';
    backdrop.innerHTML =
        '<div class="modal media-picker-modal">' +
            '<div class="modal-title">选择媒体</div>' +
            '<div class="modal-body"><div class="media-picker-grid" id="media-picker-grid">加载中...</div></div>' +
            '<div class="modal-actions">' +
                '<button class="btn btn-secondary" id="media-picker-cancel">取消</button>' +
                '<div style="flex:1"></div>' +
                '<input type="text" id="media-picker-url" placeholder="或输入图片 URL..." class="form-input" style="width:240px;margin-right:8px;">' +
                '<button class="btn btn-primary" id="media-picker-insert-url">插入 URL</button>' +
            '</div>' +
        '</div>';
    document.body.appendChild(backdrop);

    document.getElementById('media-picker-cancel').onclick = () => backdrop.remove();
    backdrop.onclick = (e) => { if (e.target === backdrop) backdrop.remove(); };

    document.getElementById('media-picker-insert-url').onclick = () => {
        const url = document.getElementById('media-picker-url').value.trim();
        if (url) {
            editor.chain().focus().setImage({ src: url }).run();
            backdrop.remove();
        }
    };

    fetch('/admin/api/media')
        .then(r => r.json())
        .then(items => {
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
                el.onclick = () => {
                    editor.chain().focus().setImage({ src: item.url }).run();
                    backdrop.remove();
                };
                grid.appendChild(el);
            });
        })
        .catch(() => {
            document.getElementById('media-picker-grid').innerHTML =
                '<p style="text-align:center;color:var(--c-danger);">加载媒体失败</p>';
        });
}
