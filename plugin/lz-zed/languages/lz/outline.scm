; ---------------------------------------------------------------------------
; Lang-Zone (lz) outline queries — feeds the Zed outline panel
; ---------------------------------------------------------------------------

(function_definition name: (_) @function)
(iterator_definition name: (identifier) @function)
(macro_definition name: (identifier) @function)
(template_definition name: (identifier) @function)
(struct_definition name: (identifier) @type)
(enum_definition name: (identifier) @type)
(trait_definition name: (identifier) @type)
(duck_definition name: (identifier) @type)
(type_alias name: (identifier) @type)
(const_definition name: (identifier) @constant)
(suite_definition name: (identifier) @type)
(block_statement name: (identifier) @label)
(magic_method) @function.magic
