<template>
    <div class="clickable">
        <b-form-radio-group
            v-if="tokens.length > 0"
            class="w-100"
            button-variant="outline-info"
            id="btn-radios-1"
            v-model="selected"
            :options="tokens"
            buttons
            name="radios-btn-default"
        ></b-form-radio-group>
        <p v-else>
            <b>You don't have any tokens yet.</b>
        </p>
    </div>
</template>

<script>
export default {
    name: 'TokenSelector',
    props: ['tokens'],
    data: () => ({
        selected: null,
    }),
    created() {
        console.log('TokenSelector component created, tokens:', this.tokens);
        this.maybeSetDefaultToken();
    },
    watch: {
        tokens: function() {
            this.maybeSetDefaultToken();
        },
        selected: function () {
            this.$emit('update:selected', this.selected);
        },
    },
    methods: {
        maybeSetDefaultToken() {
            if (this.selected == null && this.tokens.length > 0) {
                this.selected = this.tokens[0];
            }
        },
    },
}
</script>
