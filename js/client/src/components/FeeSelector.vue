<template>
    <div>
        <b-form-radio-group
            class="w-100"
            button-variant="outline-info"
            v-model="selected"
            :options="feesOptions"
            buttons
        ></b-form-radio-group>
    </div>
</template>

<script>
export default {
    name: 'TokenSelector',
    props: ['fees'],
    data: () => ({
        feesOptions: [],
        selected: 0,
    }),
    created() {
        this.updateRenderableFees();
        this.$emit('update:selected', this.selected);
    },
    watch: {
        fees() {
            this.updateRenderableFees();
        },
        selected() {
            this.$emit('update:selected', this.selected);
        },
    },
    methods: {
        updateRenderableFees() {
            this.feesOptions = this.fees
                .map((text, i) => ({
                    text, 
                    value: i,
                }));
        },
    },
}
</script>
