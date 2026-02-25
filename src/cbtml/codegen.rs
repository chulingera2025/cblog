use crate::cbtml::parser::{self, Node};
use anyhow::Result;

/// 将 AST 转换为 MiniJinja 模板字符串
pub fn generate(ast: &Node) -> Result<String> {
    let mut output = String::new();
    generate_node(ast, &mut output, 0)?;
    Ok(output)
}

fn generate_node(node: &Node, output: &mut String, _depth: usize) -> Result<()> {
    match node {
        Node::Document { extends, children } => {
            if let Some(parent) = extends {
                // 跨主题继承：aurora:post → aurora/post.cbtml
                let template_path = if let Some((theme, template)) = parent.split_once(':') {
                    format!("{}/{}.cbtml", theme, template)
                } else {
                    format!("{}.cbtml", parent)
                };
                output.push_str(&format!("{{% extends \"{}\" %}}\n", template_path));
            }
            for child in children {
                generate_node(child, output, 0)?;
            }
        }
        Node::Element {
            tag,
            classes,
            id,
            attributes,
            children,
            self_closing,
        } => {
            output.push('<');
            output.push_str(tag);

            if !classes.is_empty() {
                output.push_str(&format!(" class=\"{}\"", classes.join(" ")));
            }
            if let Some(id) = id {
                output.push_str(&format!(" id=\"{}\"", id));
            }
            for (key, value) in attributes {
                output.push_str(&format!(" {}=\"{}\"", key, value));
            }

            if *self_closing || parser::is_void_element(tag) {
                output.push_str(" />");
            } else {
                output.push('>');
                for child in children {
                    generate_node(child, output, _depth + 1)?;
                }
                output.push_str(&format!("</{}>", tag));
            }
        }
        Node::Text(text) => {
            output.push_str(text);
        }
        Node::Expression(expr) => {
            output.push_str(&format!("{{{{ {} }}}}", expr));
        }
        Node::Raw(expr) => {
            output.push_str(&format!("{{{{ {} | safe }}}}", expr));
        }
        Node::Conditional {
            condition,
            then_branch,
            else_if_branches,
            else_branch,
        } => {
            output.push_str(&format!("{{% if {} %}}", condition));
            for child in then_branch {
                generate_node(child, output, _depth)?;
            }
            for (cond, branch) in else_if_branches {
                output.push_str(&format!("{{% elif {} %}}", cond));
                for child in branch {
                    generate_node(child, output, _depth)?;
                }
            }
            if let Some(else_branch) = else_branch {
                output.push_str("{% else %}");
                for child in else_branch {
                    generate_node(child, output, _depth)?;
                }
            }
            output.push_str("{% endif %}");
        }
        Node::ForLoop {
            var,
            collection,
            body,
        } => {
            output.push_str(&format!("{{% for {} in {} %}}", var, collection));
            for child in body {
                generate_node(child, output, _depth)?;
            }
            output.push_str("{% endfor %}");
        }
        Node::Slot { name, children } => {
            output.push_str(&format!("{{% block {} %}}", name));
            for child in children {
                generate_node(child, output, _depth)?;
            }
            output.push_str(&format!("{{% endblock {} %}}", name));
        }
        Node::Include(path) => {
            output.push_str(&format!("{{% include \"{}.cbtml\" %}}", path));
        }
        Node::Style(content) => {
            output.push_str("<style>");
            output.push_str(content);
            output.push_str("</style>");
        }
        Node::Script(content) => {
            output.push_str("<script>");
            output.push_str(content);
            output.push_str("</script>");
        }
        Node::Comment(_) => {
            // 注释不输出
        }
        Node::Hook { name, data } => {
            // hook 调用映射为 MiniJinja 函数调用
            if data.is_empty() {
                output.push_str(&format!("{{{{ hook(\"{}\") }}}}", name));
            } else {
                output.push_str(&format!("{{{{ hook(\"{}\", {}) }}}}", name, data));
            }
        }
    }
    Ok(())
}
