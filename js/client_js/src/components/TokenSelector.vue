<template>
    <div>
        <b-form-radio-group
            class="w-100"
            button-variant="outline-info"
            id="btn-radios-1"
            v-model="selected"
            :options="tokens"
            buttons
            name="radios-btn-default"
        ></b-form-radio-group>
    </div>
</template>

<script>
export default {
    name: 'TokenSelector',
    props: ['tokens'],
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
