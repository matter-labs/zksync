/** @type {import("prettier").Config} */
module.exports = {
    // Basic formatting rules
    tabWidth: 4,
    printWidth: 120,
    parser: 'typescript',
    
    // Coding style & quotes
    singleQuote: true,
    bracketSpacing: true,
    
    // Advanced optimizations for Git and Collaboration
    // Changed from "none" to "all" to prevent unnecessary diff noise in Git
    trailingComma: 'all', 
    
    // Consistency across different OS (Windows, Linux, macOS)
    endOfLine: 'lf',
    
    // Ensures parentheses are always used for arrow functions, aiding code clarity
    arrowParens: 'always'
};
