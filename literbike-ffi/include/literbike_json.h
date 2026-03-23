/**
 * @file literbike_json.h
 * @brief C ABI bindings for Literbike JSON Parser
 *
 * This header provides thread-safe JSON parsing functions to replace
 * Bun's race-condition-prone HashMapPool implementation.
 *
 * @version 0.1.0
 * @license AGPL-3.0
 *
 * # Thread Safety
 *
 * All functions in this API are thread-safe and can be called concurrently
 * from multiple threads without external synchronization.
 *
 * # Memory Management
 *
 * - All pointers returned by parse functions must be freed with
 *   literbike_json_free()
 * - All strings returned must be freed with literbike_json_string_free()
 * - Never free pointers returned by literbike_json_last_error()
 *
 * # Example Usage
 *
 * ```c
 * #include "literbike_json.h"
 *
 * int main() {
 *     const char* json = "{\"name\": \"value\", \"count\": 42}";
 *     void* ast = literbike_json_parse(json);
 *
 *     if (ast == NULL) {
 *         const char* error = literbike_json_last_error();
 *         fprintf(stderr, "Parse error: %s\n", error);
 *         return 1;
 *     }
 *
 *     // Get type code
 *     int type = literbike_json_type(ast);
 *     printf("Type: %d\n", type);  // 0 = object
 *
 *     // Serialize back to string
 *     char* str = literbike_json_to_string(ast);
 *     printf("JSON: %s\n", str);
 *     literbike_json_string_free(str);
 *
 *     // Clean up
 *     literbike_json_free(ast);
 *     return 0;
 * }
 * ```
 */

#ifndef LITERBIKE_JSON_H
#define LITERBIKE_JSON_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Opaque handle to parsed JSON AST
 *
 * This represents a parsed JSON value. The internal structure is
 * intentionally hidden - use the accessor functions to inspect values.
 */
typedef void JsonAst;

/**
 * @brief Parse a JSON string
 *
 * Thread-safe JSON parsing with proper memory management and error handling.
 * Replaces Bun's thread-unsafe HashMapPool implementation.
 *
 * @param json_str Null-terminated UTF-8 JSON string
 * @return Opaque pointer to parsed AST, or NULL on error
 *
 * @note Must be freed with literbike_json_free()
 * @see literbike_json_last_error() for error details
 * @see literbike_json_parse5() for JSON5 support
 */
JsonAst* literbike_json_parse(const char* json_str);

/**
 * @brief Parse a JSON5 string
 *
 * JSON5 allows:
 * - Comments (// single-line and /* multi-star *\/)
 * - Trailing commas
 * - Unquoted object keys
 * - Single-quoted strings
 *
 * @param json_str Null-terminated UTF-8 JSON5 string
 * @return Opaque pointer to parsed AST, or NULL on error
 *
 * @note Must be freed with literbike_json_free()
 * @see literbike_json_last_error() for error details
 */
JsonAst* literbike_json_parse5(const char* json_str);

/**
 * @brief Free a parsed JSON AST
 *
 * Safely deallocates memory from parse functions.
 *
 * @param ast Pointer to AST from parse function (ignored if NULL)
 *
 * @note Pointer becomes invalid after this call
 * @note Double-free causes undefined behavior
 */
void literbike_json_free(JsonAst* ast);

/**
 * @brief Get the last error message
 *
 * Returns a human-readable error message for the last parse error.
 * The returned pointer is valid until the next call to parse functions.
 *
 * @return Pointer to null-terminated error string, or empty string if no error
 *
 * @note Do not free the returned pointer
 * @note Thread-local storage (different threads have separate errors)
 */
const char* literbike_json_last_error(void);

/**
 * @brief Serialize JSON AST to string
 *
 * Converts the parsed AST back to a JSON string for debugging or inspection.
 *
 * @param ast Pointer to AST from parse function
 * @return Newly allocated null-terminated JSON string, or NULL on error
 *
 * @note Must be freed with literbike_json_string_free()
 * @note Returns NULL if ast is NULL or serialization fails
 */
char* literbike_json_to_string(JsonAst* ast);

/**
 * @brief Free a string from literbike_json_to_string()
 *
 * @param str String pointer (ignored if NULL)
 */
void literbike_json_string_free(char* str);

/**
 * @brief Get JSON value type
 *
 * Returns a type code for the AST node.
 *
 * @param ast Pointer to AST from parse function
 * @return Type code:
 *         - 0: Object
 *         - 1: Array
 *         - 2: String
 *         - 3: Number
 *         - 4: Boolean
 *         - 5: Null
 *         - -1: NULL ast pointer
 */
int literbike_json_type(JsonAst* ast);

/**
 * @brief Type codes for literbike_json_type()
 */
typedef enum {
    LITERBIKE_JSON_OBJECT = 0,
    LITERBIKE_JSON_ARRAY = 1,
    LITERBIKE_JSON_STRING = 2,
    LITERBIKE_JSON_NUMBER = 3,
    LITERBIKE_JSON_BOOLEAN = 4,
    LITERBIKE_JSON_NULL = 5,
    LITERBIKE_JSON_INVALID = -1
} LiterbikeJsonType;

#ifdef __cplusplus
}
#endif

#endif /* LITERBIKE_JSON_H */
