<template>
    <div>
        <b-form-radio-group
            class="w-100"
            button-variant="outline-info"
            v-model="selected"
            :options="fees"
            buttons
        ></b-form-radio-group>
    </div>
</template>

<script>
export default {
    name: 'TokenSelector',
    props: ['fees'],
    data: () => ({
        selected: null,
    }),
    watch: {
        selected: function () {
            this.$emit('update:selected', this.selected);
        },
    },
}
</script>
