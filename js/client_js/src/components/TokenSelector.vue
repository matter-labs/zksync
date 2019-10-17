<template>
    <div class="clickable">
        <img v-if="disabledReason == 'Not loaded'" style="margin-right: 1.5em" src="../assets/loading.gif" width="100em">
        <p v-else-if="disabledReason">
            <b> {{ disabledReason }} </b>
        </p>
        <b-form-radio-group
            v-else
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
    created() {
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
    computed: {
        disabledReason() {
            return this.tokens == null     ? "Not loaded"
                 : this.tokens.length == 0 ? "You don't have any tokens yet."
                 : null;
        },
    },
    methods: {
        maybeSetDefaultToken() {
            if (this.selected == null && this.tokens && this.tokens.length > 0) {
                this.selected = this.tokens[0];
            }
        },
    },
}
</script>
