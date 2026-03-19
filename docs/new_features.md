# New Features and Transformation Ideas

This document outlines potential features and transformations that could be incorporated into reformat. These suggestions range from simple enhancements to complex code restructuring capabilities.

## Priority Classification

- [green] **Easy**: Low complexity, high value additions
- [yellow] **Medium**: Moderate complexity, good value
-  **Complex**: High complexity, specialized use cases

---

## 1. Identifier Transformations

### [green] Prefix/Suffix Operations

**Strip Prefix/Suffix**
```rust
// Before
m_userName, m_userId, m_value
old_function, old_method

// After (strip "m_", "old_")
userName, userId, value
function, method
```

**Replace Prefix/Suffix**
```rust
// Before
IUserService, IDataProvider

// After (replace "I" with "Abstract")
AbstractUserService, AbstractDataProvider
```

**Implementation**: Extend `CaseConverter` with `PrefixAction` enum

### [green] Boolean Naming Conventions

Add or normalize boolean prefixes:
```rust
// Add prefixes
active → isActive
enabled → hasEnabled
visible → shouldShow

// Normalize prefixes
is_active → isActive (with case conversion)
has_value → hasValue
```

**Use cases**:
- Standardize boolean naming across codebase
- Language-specific conventions (Java: `is*`, Python: `is_*`)

### [yellow] Pluralization/Singularization

```python
# Before
user = get_all_users()
items = fetch_item()

# After
users = get_all_users()
item = fetch_item()
```

**Implementation**: Requires inflection library or rules database

### [yellow] Abbreviation Expansion/Contraction

```javascript
// Expand
btn → button
msg → message
usr → user

// Contract
button → btn
message → msg
```

**Configuration**: User-defined abbreviation dictionary

---

## 2. String and Literal Transformations

### [green] Quote Style Conversion

**Single ↔ Double Quotes**
```javascript
// Before
let name = 'Alice';
const value = "test";

// After
let name = "Alice";
const value = "test";
```

**Template Literals (JavaScript/TypeScript)**
```javascript
// Before
const greeting = "Hello, " + name + "!";

// After
const greeting = `Hello, ${name}!`;
```

**Implementation**: Language-aware string detection with escape handling

### [green] String Format Modernization

**Python f-strings**
```python
# Before
name = "Alice"
print("Hello, {}".format(name))
print("Value: %s" % value)

# After
print(f"Hello, {name}")
print(f"Value: {value}")
```

**Rust format macros**
```rust
// Modernize to newer macro syntax
println!("{:?}", value);  // Ensure consistent formatting
```

### [yellow] Number Literal Formatting

```rust
// Before
let x = 1000000;
let y = 0xFF;

// After (with separators)
let x = 1_000_000;
let y = 0xFF;
```

---

## 3. Code Style Transformations

### [green] Whitespace Normalization

**Tabs ↔ Spaces**
```python
# Convert tabs to 4 spaces or vice versa
def function():
→   return value  # Tab

def function():
    return value  # 4 spaces
```

**Line Ending Normalization**
- CRLF (Windows) ↔ LF (Unix)
- Trailing whitespace removal
- Consistent empty lines

### [green] Comment Style Conversion

**C-style languages**
```c
// Before (inline comments)
int x = 5;  // This is x

/* After (block comments) */
int x = 5;  /* This is x */
```

**Python docstrings**
```python
# Before (single quotes)
def foo():
    '''This is a docstring'''
    pass

# After (double quotes)
def foo():
    """This is a docstring"""
    pass
```

---

## 4. Import/Module Management

### [yellow] Import Organization

**Sorting and Grouping**
```python
# Before
import os
from myapp import utils
import sys
from third_party import lib

# After (grouped and sorted)
import os
import sys

from third_party import lib

from myapp import utils
```

**Groups**: Standard library, Third-party, Local imports

### [yellow] Import Style Conversion

**JavaScript/TypeScript**
```javascript
// Before (CommonJS)
const fs = require('fs');
const path = require('path');

// After (ES6)
import fs from 'fs';
import path from 'path';
```

**Python**
```python
# Before (star imports)
from module import *

# After (explicit imports)
from module import func1, func2, Class1
```

### [green] Remove Unused Imports

```python
# Before
import os
import sys
import json  # Unused

def main():
    print(os.path.exists("file.txt"))
```

Detect and remove `json` import.

---

## 5. Language-Specific Modernization

### [yellow] JavaScript/TypeScript

**var → let/const**
```javascript
// Before
var name = "Alice";
var count = 0;

// After (with scope analysis)
const name = "Alice";
let count = 0;
```

**Function → Arrow Functions**
```javascript
// Before
const add = function(a, b) {
    return a + b;
};

// After
const add = (a, b) => a + b;
```

**Promises → Async/Await**
```javascript
// Before
function fetchData() {
    return fetch(url)
        .then(response => response.json())
        .then(data => processData(data));
}

// After
async function fetchData() {
    const response = await fetch(url);
    const data = await response.json();
    return processData(data);
}
```

### [yellow] Python Modernization

**Type Hints Addition**
```python
# Before
def add(a, b):
    return a + b

# After
def add(a: int, b: int) -> int:
    return a + b
```

**Walrus Operator (Python 3.8+)**
```python
# Before
data = fetch_data()
if data:
    process(data)

# After
if (data := fetch_data()):
    process(data)
```

### Rust Modernization

**unwrap() → ? operator**
```rust
// Before
let value = some_function().unwrap();
let result = another_function().unwrap();

// After
let value = some_function()?;
let result = another_function()?;
```

**match → if let**
```rust
// Before
match some_option {
    Some(value) => process(value),
    None => {}
}

// After
if let Some(value) = some_option {
    process(value);
}
```

---

## 6. Structural Pattern Transformations

### Getter/Setter Pattern Detection

```java
// Before (verbose Java)
private String name;

public String getName() {
    return this.name;
}

public void setName(String name) {
    this.name = name;
}

// After (modern language feature)
public String name { get; set; }  // C#
@property  // Python decorator
```

### Builder Pattern → Constructor

```javascript
// Before
const user = new UserBuilder()
    .setName("Alice")
    .setAge(30)
    .setEmail("alice@example.com")
    .build();

// After
const user = new User({
    name: "Alice",
    age: 30,
    email: "alice@example.com"
});
```

### Callback → Promise → Async/Await

```javascript
// Stage 1: Callback
function fetchUser(id, callback) {
    db.query('SELECT * FROM users WHERE id = ?', [id], callback);
}

// Stage 2: Promise
function fetchUser(id) {
    return new Promise((resolve, reject) => {
        db.query('SELECT * FROM users WHERE id = ?', [id], (err, result) => {
            if (err) reject(err);
            else resolve(result);
        });
    });
}

// Stage 3: Async/Await
async function fetchUser(id) {
    return await db.query('SELECT * FROM users WHERE id = ?', [id]);
}
```

---

## 7. API Migration and Deprecation

### [yellow] Deprecated API Replacement

**Configurable Mappings**
```yaml
# migration-config.yaml
replacements:
  - old: "$.ajax"
    new: "fetch"
    type: "function_call"

  - old: "moment()"
    new: "dayjs()"
    type: "library"

  - old: "React.createClass"
    new: "class extends React.Component"
    type: "structural"
```

**Examples**:
```javascript
// Before
$.ajax({
    url: '/api/users',
    success: function(data) { ... }
});

// After
fetch('/api/users')
    .then(response => response.json())
    .then(data => { ... });
```

### [yellow] Framework Migration Helpers

**React Class → Functional Components**
```javascript
// Before
class UserProfile extends React.Component {
    constructor(props) {
        super(props);
        this.state = { count: 0 };
    }

    render() {
        return <div>{this.state.count}</div>;
    }
}

// After
function UserProfile(props) {
    const [count, setCount] = useState(0);
    return <div>{count}</div>;
}
```

---

## 8. Code Quality Improvements

### [green] Magic Number/String Extraction

```python
# Before
def calculate_tax(amount):
    return amount * 0.15

def validate_age(age):
    return age >= 18

# After
TAX_RATE = 0.15
MINIMUM_AGE = 18

def calculate_tax(amount):
    return amount * TAX_RATE

def validate_age(age):
    return age >= MINIMUM_AGE
```

**Detection**:
- Identify repeated literals
- Suggest constant extraction
- Generate constant names

### [yellow] Boolean Expression Simplification

```javascript
// Before
if (x == true) { }
if (enabled == false) { }
if (!!value) { }

// After
if (x) { }
if (!enabled) { }
if (value) { }
```

### [yellow] Ternary ↔ if/else Conversion

```javascript
// Ternary → if/else
// Before
const result = condition ? valueA : valueB;

// After
let result;
if (condition) {
    result = valueA;
} else {
    result = valueB;
}

// Vice versa for simple cases
```

---

## 9. Documentation Transformations

### [yellow] Comment Format Conversion

**JSDoc ↔ TypeDoc**
```javascript
// Before (JSDoc)
/**
 * @param {string} name - User name
 * @returns {User} User object
 */

// After (TypeDoc)
/**
 * @param name - User name
 * @returns User object
 */
```

**Generate Documentation from Signatures**
```python
# Before
def calculate_total(items, tax_rate):
    return sum(items) * (1 + tax_rate)

# After (with generated docstring)
def calculate_total(items, tax_rate):
    """
    Calculate total with tax.

    Args:
        items: List of item prices
        tax_rate: Tax rate as decimal

    Returns:
        Total price including tax
    """
    return sum(items) * (1 + tax_rate)
```

---

## 10. Advanced Pattern Matching

### Semantic Code Search

**AST-based Pattern Matching**
```javascript
// Find all: functions that don't handle errors
function fetchData() {
    fetch(url).then(data => process(data));
    // Missing .catch()
}

// Suggest: Add error handling
function fetchData() {
    fetch(url)
        .then(data => process(data))
        .catch(err => handleError(err));
}
```

### Code Clone Detection

Identify duplicated code blocks and suggest extraction:
```python
# Detect duplicated validation logic
def validate_user(user):
    if not user.name:
        raise ValueError("Name required")
    if not user.email:
        raise ValueError("Email required")

def validate_product(product):
    if not product.name:
        raise ValueError("Name required")
    if not product.price:
        raise ValueError("Price required")

# Suggest: Extract common validation
def require_field(obj, field, message):
    if not getattr(obj, field):
        raise ValueError(message)
```

---

## 11. Performance Optimizations

### [yellow] List Comprehension Conversion (Python)

```python
# Before
result = []
for item in items:
    if item.is_valid():
        result.append(item.value)

# After
result = [item.value for item in items if item.is_valid()]
```

### [yellow] Stream API (Java)

```java
// Before
List<String> result = new ArrayList<>();
for (User user : users) {
    if (user.isActive()) {
        result.add(user.getName());
    }
}

// After
List<String> result = users.stream()
    .filter(User::isActive)
    .map(User::getName)
    .collect(Collectors.toList());
```

---

## 12. Testing and Quality Assurance

### [yellow] Generate Test Stubs

```python
# From this function
def process_user(user_id, action):
    user = fetch_user(user_id)
    if action == "activate":
        user.activate()
    return user

# Generate test template
def test_process_user_activate():
    # Arrange
    user_id = 1
    action = "activate"

    # Act
    result = process_user(user_id, action)

    # Assert
    assert result.is_active == True
```

---

## Implementation Roadmap

### Phase 1: Core Enhancements (Easy Wins)
1. Quote style conversion
2. Prefix/suffix strip/replace
3. Whitespace normalization
4. Boolean prefix operations
5. Import sorting

### Phase 2: Language Modernization (Medium)
1. JavaScript var → let/const
2. Python f-string conversion
3. Import style conversion
4. Type hint addition (Python)
5. Number literal formatting

### Phase 3: Advanced Transformations (Complex)
1. Callback → Promise → Async/Await
2. React class → functional components
3. AST-based pattern matching
4. Getter/setter pattern transformation
5. Code clone detection

### Phase 4: AI-Assisted (Future)
1. Intelligent code refactoring suggestions
2. Context-aware API migrations
3. Automated test generation
4. Code quality improvements with ML

---

## Configuration Examples

### Feature Toggle

```yaml
# reformat-config.yaml
transformers:
  case_conversion:
    enabled: true

  quote_style:
    enabled: true
    target: "double"

  import_organization:
    enabled: true
    groups: [stdlib, external, local]

  modernization:
    javascript:
      var_to_let_const: true
      arrow_functions: true
    python:
      f_strings: true
      type_hints: false  # Opt-in
```

### Custom Transformation Rules

```yaml
custom_rules:
  - name: "hungarian_notation_removal"
    pattern: "^(str|int|arr|obj)([A-Z].*)$"
    replacement: "$2"
    scope: "variables"

  - name: "api_migration"
    find: "oldApi.fetchData"
    replace: "newApi.getData"
    files: "**/*.js"
```

---

## Contributing New Features

When adding new transformations to reformat:

1. **Create module** in `reformat-core/src/transformers/<feature>.rs`
2. **Implement Transformer trait** with clear transformation logic
3. **Add comprehensive tests** in module and integration tests
4. **Document behavior** in module docs and examples
5. **Update pipeline builder** with convenience method
6. **Add CLI flags** if user-facing
7. **Update CHANGELOG.md** with feature description

---

## Feature Request Template

```markdown
### Feature Name
Brief description of transformation

**Priority**: [green]/[yellow]/

**Use Case**:
Describe when users would need this

**Example**:
```<language>
// Before
<code>

// After
<code>
```

**Implementation Notes**:
- Dependencies needed
- Complexity estimates
- Edge cases to consider

**Related Features**:
Links to similar transformations
```

---

## Feedback and Suggestions

Have ideas for new transformations? Open an issue at:
https://github.com/yourusername/reformat/issues

Label: `enhancement`, `transformation-idea`
