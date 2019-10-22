<template>
    <b-container v-if="pendingOps && pendingOps.length">
        <div class="mb-1">
            <b-row>
                <b-col class="px-0 w-100" style="text-align: center">
                    <h6>You have operations not yet completed.</h6>
                </b-col>
            </b-row>
            <b-row v-for="op in pendingOps" :key="op.elem_id" class="mb-2">
                <CompleteOperationButton
                    class="w-100"
                    :op="op"
                    v-on:completionSuccess="completionSuccess"
                    ></CompleteOperationButton>
            </b-row>
        </div>
    </b-container>
</template>

<script>
import CompleteOperationButton from './CompleteOperationButton.vue';

const components = {
    CompleteOperationButton,
};

export default {
    name: 'PendingOperationNotifier',
    data: () => ({
        pendingOps: null,
    }),
    created() {
        this.updatePendingOps();
    },
    methods: {
        updatePendingOps() {
            let pendingOps = window.walletDecorator.pendingOperationsAsRenderableList();
            this.pendingOps = pendingOps;
        },
        completionSuccess(...args) {
            this.updatePendingOps();
        },
    },
    components,
}
</script>
