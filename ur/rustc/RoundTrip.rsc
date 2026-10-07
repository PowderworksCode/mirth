module rustc::RoundTrip

// Patterns behind three incremental-compilation bugs, generalized so that each
// still finds the bug it came from (docs/hunt/issue-*.md in mirth).

data Language = language(str name);
language("rust");

data Classify = classify(str function, str description);
classify("hashOrderEncoded", "a hash-ordered collection in a type that derives an encoder: written in iteration order, which a round trip can change");
classify("hashOrderAlias", "a type alias for a hash-ordered collection: anything encoding a value of it writes iteration order");
classify("decodedFresh", "a decoder that reserves a fresh identity, where creation may have deduplicated");
classify("untrackedWhileEncoding", "an encoder reading untracked state: the session, its source map, the environment, the clock");

data Rewrite = rewrite(str function, str description);
rewrite("never", "rewrites nothing");
str never((Atom)`NEVER_MATCHES_ANYTHING`) = "never";

set[str] hashOrdered = {"FxHashMap", "FxHashSet", "HashMap", "HashSet", "UnordMap", "UnordSet"};

bool derivesEncoder(node item) = any(a <- item.attributes, contains(unparse(a), "derive"), contains(unparse(a), "Encodable"));

str hashOrderEncoded(FieldDeclaration f) = owner + "." + unparse(f.name) + ": " + kind
  when let items = [a | a <- ancestors(f), a is Item],
       size(items) > 0,
       let item = items[0],
       derivesEncoder(item),
       let ty = unparse(f.type),
       let found = [k | k <- hashOrdered, contains(ty, k + "<")],
       size(found) > 0,
       let kind = found[0],
       let owner = (item.item has name) ? unparse(item.item.name) : "?";

str hashOrderAlias(TypeAlias t) = unparse(t.name) + " = " + kind
  when let ty = unparse(t.type),
       let found = [k | k <- hashOrdered, contains(ty, k + "<")],
       size(found) > 0,
       let kind = found[0];

bool inDecoder(node n) = any(a <- ancestors(n), a is Function, contains(toLowerCase(unparse(a.name)), "decode"));

str decodedFresh(MethodCall c) = name
  when let name = unparse(c.method),
       startsWith(name, "reserve") || startsWith(name, "fresh") || contains(name, "next_id"),
       !contains(name, "dedup"),
       !deduplicatesInThisFile(name),
       inDecoder(c);

// A function defined in the same file whose body deduplicates is not fresh.
bool deduplicatesInThisFile(str name) = any(f <- descendants(root()), f is Function, unparse(f.name) == name, contains(unparse(f.body), "dedup"));

bool inEncoder(node n) = any(a <- ancestors(n), (a is Function && startsWith(unparse(a.name), "encode")) || (a is Impl && contains(unparse(a.type), "Encode")));

bool isSession(node o) = o is Field && unparse(o.field) == "sess";

str untrackedWhileEncoding(Field f) = "sess." + unparse(f.field)
  when isSession(f.operand), inEncoder(f);
str untrackedWhileEncoding(MethodCall c) = "sess." + unparse(c.method) + "()"
  when isSession(c.operand), inEncoder(c);
str untrackedWhileEncoding(Call c) = callee
  when let callee = unparse(c.operand),
       endsWith(callee, "env::var") || endsWith(callee, "env::var_os") || callee == "SystemTime::now" || callee == "Instant::now",
       inEncoder(c);
