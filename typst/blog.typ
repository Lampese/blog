// Import once in an article: #import "/typst/blog.typ": *
// The generator supplies fixed canonical labels; localization happens after projection.
#let canonical-text = json(bytes(sys.inputs.at("blog-text", default: "{}")))
#let blog-label-text(key, fallback) = canonical-text.at(key, default: fallback)
#let blog-label(key, fallback) = html.elem("span", attrs: ("data-blog-i18n": key), blog-label-text(key, fallback))

#let post(id: none, title: none, date: none, summary: "", tags: (), draft: false) = {
  [#metadata((id: id, title: title, date: date, summary: summary, tags: tags, draft: draft)) <blog-post>]
}

#let site-page(title: none, summary: "") = {
  [#metadata((title: title, summary: summary)) <blog-page>]
}

#let experience-position(role, from: none, until: none, until-note: none, team: none) = {
  assert(from != none, message: "An experience position requires a start date")
  html.elem("li", attrs: (class: "experience-position", role: "listitem"))[
    #html.elem("div", attrs: (class: "experience-description"))[
      #html.elem("span", attrs: (class: "experience-role"), role)
      #if team != none { html.elem("span", attrs: (class: "experience-team"), team) }
    ]
    #html.elem("p", attrs: (class: "experience-dates"))[
      #html.elem("time", attrs: (datetime: from), from.replace("-", "."))
      #html.elem("span")[ — ]
      #if until == none { blog-label("present", "Present") } else {
        html.elem("time", attrs: (datetime: until), until.replace("-", "."))
      }
      #if until-note != none { html.elem("span", attrs: (class: "experience-date-note"), until-note) }
    ]
  ]
}

#let experience(name, url: none, logos: (), description: none, body) = {
  html.elem("section", attrs: (class: "experience-group"))[
    #html.elem("header", attrs: (class: "experience-organization"))[
      #html.elem("span", attrs: (class: "experience-marks", "aria-hidden": "true"))[
        #for logo in logos {
          html.elem("img", attrs: (
            src: logo, alt: "", width: "32", height: "32",
            loading: "lazy", decoding: "async",
          ))
        }
      ]
      #html.elem("h3")[#if url == none { name } else { link(url, name) }]
    ]
    #if description != none {
      html.elem("p", attrs: (class: "experience-summary"), description)
    }
    #html.elem("ul", attrs: (class: "experience-positions", role: "list"), body)
  ]
}

#let education(name, from: none, until: none, note: none, url: none, logo: none) = {
  assert(from != none and until != none, message: "Education requires start and end dates")
  html.elem("section", attrs: (class: "education-entry"))[
    #html.elem("span", attrs: (class: "education-mark", "aria-hidden": "true"))[
      #if logo != none {
        html.elem("img", attrs: (src: logo, alt: "", width: "32", height: "32", loading: "lazy", decoding: "async"))
      }
    ]
    #html.elem("div", attrs: (class: "education-main"))[
      #html.elem("h3")[#if url == none { name } else { link(url, name) }]
      #html.elem("p", attrs: (class: "education-period"))[
        #html.elem("time", attrs: (datetime: from), from.replace("-", "."))
        #html.elem("span")[ — ]
        #html.elem("time", attrs: (datetime: until), until.replace("-", "."))
        #if note != none { html.elem("span", attrs: (class: "education-annotation"), note) }
      ]
    ]
  ]
}

#let excerpt(reference) = {
  [#metadata(reference) <blog-reference>]
  html.elem("blog-excerpt", attrs: ("data-reference": json.encode(reference)))
}

#let statement(kind, title: none, label-key: none, body) = {
  let name = if label-key == none { [#kind] } else { blog-label(label-key, kind) }
  figure(
    html.elem("section", attrs: (class: "statement"))[
      #context html.elem("p", attrs: (class: "statement-title"))[#name #counter(figure.where(kind: kind)).display()#if title != none [#blog-label("statement_title_open", " (")#title#blog-label("statement_title_close", ")")]]
      #body
    ], kind: kind, supplement: name, numbering: "1", outlined: false,
  )
}
#let theorem(title: none, body) = statement(blog-label-text("theorem", "Theorem"), title: title, label-key: "theorem", body)
#let lemma(title: none, body) = statement(blog-label-text("lemma", "Lemma"), title: title, label-key: "lemma", body)
#let definition(title: none, body) = statement(blog-label-text("definition", "Definition"), title: title, label-key: "definition", body)
#let proof(body) = html.elem("section", attrs: (class: "proof"))[#emph[#blog-label("proof", "Proof.")] #body #html.elem("span", attrs: (class: "qed"))[□]]
#let aside(body) = html.elem("aside", attrs: (class: "margin-note"), body)

#let equation(body) = figure(
  html.elem("div", attrs: (class: "numbered-equation"))[
    #html.elem("div", attrs: (class: "equation-body"), body)
    #context html.elem("span", attrs: (class: "equation-number"))[(#counter(figure.where(kind: "equation")).display())]
  ], kind: "equation", supplement: blog-label("equation", "Equation"), numbering: "(1)", outlined: false,
)
#let interactive(src, title: none, height: 360) = {
  let attrs = (
    src: src, title: if title == none { blog-label-text("interactive_title", "Interactive content") } else { title },
    height: str(height), class: "interactive-frame", loading: "lazy", sandbox: "allow-scripts",
  )
  if title == none { attrs.insert("data-blog-i18n-title", "interactive_title") }
  html.elem("iframe", attrs: attrs)
}
#let playground(code) = html.elem("iframe", attrs: (
  src: "/assets/playground.html", "data-code": code,
  title: blog-label-text("playground_title", "Runnable JavaScript example"),
  "data-blog-i18n-title": "playground_title",
  sandbox: "allow-scripts", class: "playground", height: "280",
))
