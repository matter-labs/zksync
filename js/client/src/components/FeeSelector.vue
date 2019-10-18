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
        this.renderableFees();
        this.$emit('update:selected', this.selected);
    },
    watch: {
        fees: function() {
            this.renderableFees();
        },
        selected: function () {
            this.$emit('update:selected', this.selected);
        },
    },
    methods: {
        renderableFees() {
            this.feesOptions = this.fees.map((text, value) => ({text, value}));
        },
    },
}
</script>
