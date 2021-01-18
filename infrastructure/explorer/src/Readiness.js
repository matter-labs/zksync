// TODO: When the code will be rewritten in Typescript
// this should become an enum and probably moved to another file.
//
// This object is a kinda "enum" replacement. We have to put it in a separate file
// to avoid cyclic dependencies between files which might not be resolved
// correctly.
export const Readiness = {
    Rejected: -1,
    Initiated: 0,
    Committed: 1,
    Verified: 2,
    Scheduled: 3
};
