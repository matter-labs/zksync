import { makeEntry, formatDate, readyStateFromString } from './utils';

function getBlockNumberEntry(block) {
    return makeEntry('Block number').innerHTML(block.block_number).localLink(`/blocks/${block.block_number}`);
}

function getStatusEntry(block) {
    const status = block.verified_at ? 'Verified' : 'Pending';
    const statusId = readyStateFromString(status);

    return (
        makeEntry('Status')
            // This makes it look like the previous design
            .innerHTML(' ' + status)
            .status(statusId)
    );
}

function getNewStateRootEntry(block) {
    return makeEntry('New state root')
        .innerHTML(`${block.new_state_root.slice(8, 40)}...`)
        .localLink(`/blocks/${block.block_number}`);
}

function getAcceptedAtEntry(block) {
    return makeEntry('Accepted At').innerHTML(formatDate(block.committed_at));
}

function getVerifiedAtEntry(block) {
    return makeEntry('Verified At').innerHTML(formatDate(block.verified_at));
}

export function getBlockEntries(block) {
    return {
        block_number: getBlockNumberEntry(block),
        status: getStatusEntry(block),
        new_state_root: getNewStateRootEntry(block),
        accepted_at: getAcceptedAtEntry(block),
        verified_at: getVerifiedAtEntry(block)
    };
}
