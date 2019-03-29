const txObjectProperties = ['from', 'to', 'data', 'value', 'gasPrice', 'gas'];

module.exports = hasTransactionObject;

function hasTransactionObject(args) {
  // bad/empty args: bad
  if (!Array.isArray(args) || args.length === 0) {
    return false;
  }
  const lastArg = args[args.length - 1];
  // missing or non-object: bad
  if (!lastArg) return false;
  if (typeof lastArg !== 'object') {
    return false;
  }
  // empty object: good
  if (Object.keys(lastArg).length === 0) {
    return true;
  }
  // txParams object: good
  const keys = Object.keys(lastArg);
  const hasMatchingKeys = txObjectProperties.some((value) => keys.includes(value));
  if (hasMatchingKeys) {
    return true;
  }
  // no match
  return false;
}
