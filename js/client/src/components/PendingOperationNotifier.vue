<template>
    <b-container>
        <div class="py-5">
            <b-row>
                <b-col class="px-0 w-100" style="text-align: center">
                    <h3>You have operations not yet completed.</h3>
                </b-col>
            </b-row>
            <b-row v-for="op in pendingOps" :key="op.elem_id" class="mt-3">
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
    props: ['pendingOps'],
    methods: {
        completionSuccess(...args) {
            this.$emit('completionSuccess', ...args);
        },
    },
    components,
}
</script>
