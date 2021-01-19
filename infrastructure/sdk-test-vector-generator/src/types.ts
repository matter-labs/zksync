export interface TestVectorEntry {
    inputs: any;
    outputs: any;
}

export interface TestVector<T> {
    description: string;
    items: T[];
}
