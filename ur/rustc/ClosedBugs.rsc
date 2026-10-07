module rustc::ClosedBugs

// Patterns behind closed rustc bugs (docs/motivating.md in mirth), each made as
// general as it can be while still finding the code its fix changed.

data Language = language(str name);
language("rust");

data Classify = classify(str function, str description);
classify("sortByDefId", "#82920: sorting or deduplicating by DefId, whose order is not stable across sessions");
classify("contextCache", "#89598: a cache held on a context, outside the dependency graph");
classify("untrackedCrateStore", "#84252: reading the crate store directly, which only an eval_always query may do");
classify("envRead", "#40364: reading an environment variable, which dep-info must record");
classify("fileRead", "#111227, #111295: reading a file, which dep-info and the dependency graph must record");
classify("writeInPlace", "#45841: creating or writing a file directly, rather than renaming a finished temporary file into place");
classify("writeErrorNotFatal", "#119456: a failed write reported with emit_err, after which compilation goes on and may publish a partial output");
classify("encoderNotFinished", "#117254: a function that creates a FileEncoder and never finishes it, so write errors are lost");
classify("writeResultIgnored", "#117254: the result of finishing, flushing or syncing a write thrown away");
classify("hashIterated", "#34902, #65036: iterating a hash-ordered collection declared in the same file");
classify("optionRead", "#66955: an option read where it can affect output; joined afterwards with the options marked [UNTRACKED]");

data Rewrite = rewrite(str function, str description);
rewrite("never", "rewrites nothing");
str never((Atom)`NEVER_MATCHES_ANYTHING`) = "never";

// The provider a closure is given as (`Providers { name: |tcx, key| ... }`), else the function.
str enclosing(node n) = ps[0] when let ps = [unparse(a.name) | a <- ancestors(n), a is Field, a has name, contains(unparse(a), "|")], size(ps) > 0;
str enclosing(node n) = fs[0] when let fs = [unparse(a.name) | a <- ancestors(n), a is Function], size(fs) > 0;
default str enclosing(node _) = "<no function>";

set[str] sorts = {"sort_by_key", "sort_unstable_by_key", "sort_by_cached_key", "sort_by", "sort_unstable_by", "dedup_by_key", "dedup_by", "binary_search_by_key"};

str sortByDefId(MethodCall c) = unparse(c.method) + " in " + enclosing(c)
  when unparse(c.method) in sorts,
       let args = unparse(c),
       contains(args, "def_id") || contains(args, "DefId") || contains(args, ".krate") || contains(args, "def_index") || contains(args, "local_def_index");

set[str] interior = {"Lock<", "RefCell<", "Cell<", "OnceCell<", "OnceLock<", "Sharded<", "RwLock<", "Mutex<", "AppendOnlyVec<", "FreezeLock<"};
set[str] containers = {"Map<", "Set<", "Cache", "Vec<", "Map>", "Table"};

str contextCache(FieldDeclaration f) = owner + "." + unparse(f.name)
  when let items = [a | a <- ancestors(f), a is Item],
       size(items) > 0,
       let item = items[0],
       item.item has name,
       let owner = unparse(item.item.name),
       endsWith(owner, "Ctxt") || endsWith(owner, "Context") || endsWith(owner, "Session") || owner == "CStore",
       let ty = unparse(f.type),
       any(i <- interior, contains(ty, i)),
       any(k <- containers, contains(ty, k));

str untrackedCrateStore(node c) = enclosing(c)
  when c is Call || c is MethodCall,
       let text = unparse(c),
       !contains(substring(text, 1, size(text)), "CStore::from_tcx("),
       (startsWith(text, "CStore::from_tcx(") || (c is MethodCall && unparse(c.method) in {"cstore_untracked", "untracked"}));

str envRead(Call c) = callee + " in " + enclosing(c)
  when let callee = unparse(c.operand),
       callee in {"env::var", "env::var_os", "std::env::var", "std::env::var_os", "env::vars", "std::env::vars"};

str fileRead(Call c) = callee + " in " + enclosing(c)
  when let callee = unparse(c.operand),
       callee in {"fs::read", "fs::read_to_string", "std::fs::read", "std::fs::read_to_string", "File::open", "fs::File::open", "std::fs::File::open"};

str writeInPlace(Call c) = unparse(c) + " in " + enclosing(c)
  when let callee = unparse(c.operand),
       callee in {"File::create", "fs::File::create", "std::fs::File::create", "fs::write", "std::fs::write"};

bool isWrite(str text) = any(w <- {"finish", "flush", "write", "create", "rename", "encode", "sync", "persist", "save", "emit"}, contains(text, w));

str writeErrorNotFatal(Arm a) = enclosing(a)
  when let t = unparse(a),
       startsWith(t, "Err("),
       contains(t, "emit_err("),
       let matches = [m | m <- ancestors(a), m is Match],
       size(matches) > 0,
       isWrite(unparse(matches[0].scrutinee));

str writeErrorNotFatal(If i) = enclosing(i)
  when let t = unparse(i),
       startsWith(t, "if let Err("),
       contains(t, "emit_err("),
       isWrite(t);

str encoderNotFinished(Function f) = unparse(f.name)
  when let t = unparse(f),
       contains(t, "FileEncoder::new("),
       !contains(t, "finish(");

str writeResultIgnored(Let s) = unparse(s.value) + " in " + enclosing(s)
  when unparse(s.pattern) == "_",
       s has value,
       let v = s.value,
       v is MethodCall,
       unparse(v.method) in {"finish", "flush", "sync_all", "sync_data", "write_all", "persist"};

set[str] hashOrdered = {"FxHashMap<", "FxHashSet<", "HashMap<", "HashSet<", "FnvHashMap<", "FnvHashSet<", "UnordMap<", "UnordSet<"};

// Names declared with a hash-ordered type in this file: fields, lets with a type, aliases.
set[str] hashNames() = {unparse(d.name) | d <- descendants(root()), d is FieldDeclaration, any(h <- hashOrdered, contains(unparse(d.type), h))}
  + {unparse(d.pattern) | d <- descendants(root()), d is Parameter, d has type, any(h <- hashOrdered, contains(unparse(d.type), h))}
  + {unparse(d.name) | d <- descendants(root()), d is TypeAlias, any(h <- hashOrdered, contains(unparse(d.type), h))};

str hashIterated(MethodCall c) = recv + "." + unparse(c.method) + "() in " + enclosing(c)
  when unparse(c.method) in {"iter", "into_iter", "keys", "values", "drain", "iter_mut"},
       let recv = unparse(c.operand),
       let last = lastName(recv),
       last in hashNames();

str lastName(str text) = parts[size(parts) - 1]
  when let parts = [p | p <- splitDots(text)], size(parts) > 0;

list[str] splitDots(str text) = [text];

set[str] optionHolders = {"opts", "sopts", "options", "unstable_opts", "debugging_opts", "cg"};

str optionRead(Field f) = unparse(f.field)
  when let o = f.operand,
       (o is Field && unparse(o.field) in optionHolders) || unparse(o) in optionHolders;
// Inside `impl Options`, the options are `self`.
str optionRead(Field f) = unparse(f.field)
  when unparse(f.operand) == "self",
       any(a <- ancestors(f), a is Impl, unparse(a.type) == "Options");
